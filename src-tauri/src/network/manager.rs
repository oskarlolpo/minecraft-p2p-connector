use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use quinn::{Connection, Endpoint, EndpointConfig, VarInt};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Mutex, RwLock},
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    cert::{build_insecure_client_config, build_server_config},
    models::{ConnectionState, ExternalServerProbe, NetworkStatus, PeerInfo, SessionMode},
    signaling::{discover_public_addr, punch_remote, SignalingConfig},
};

use super::{
    e4mc::{self, E4mcConfig},
    minecraft, proxy,
    wss_relay::{self, WssRelayConfig, WssRelayRuntime},
};

const ABLY_SIGNAL_LABEL: &str = "Ably Presence + Channels";
const CLIENT_CONNECT_RETRY_ATTEMPTS: usize = 10;
const CLIENT_CONNECT_TIMEOUT_MS: u64 = 1500;
const CLIENT_CONNECT_DELAY_MS: u64 = 300;
const HOST_PUNCH_GRACE_MS: u64 = 900;

#[derive(Clone)]
pub struct NetworkManager {
    inner: Arc<Inner>,
}

struct Inner {
    control: Mutex<()>,
    session: Mutex<Option<SessionRuntime>>,
    status: Arc<RwLock<NetworkStatus>>,
    stun: SignalingConfig,
    wss_relay_config: WssRelayConfig,
    e4mc: E4mcConfig,
}

struct SessionRuntime {
    cancel: CancellationToken,
    tasks: Vec<JoinHandle<()>>,
    control: SessionControl,
}

enum SessionControl {
    Host(HostControl),
    PreparedClient(PreparedClientControl),
    Client(ClientControl),
}

struct HostControl {
    punch_socket: Arc<UdpSocket>,
    room_name: String,
    peer_id: String,
    local_game_port: u16,
    expected_peers: Arc<RwLock<HashMap<SocketAddr, String>>>,
    live_connections: Arc<Mutex<HashMap<String, Connection>>>,
    relay_sessions: Arc<Mutex<HashMap<String, HostRelayRuntime>>>,
    e4mc_runtime: Option<HostE4mcRuntime>,
    upnp_mapping: Option<super::upnp::UpnpMapping>,
}

struct ClientControl {
    peer_addr: SocketAddr,
}

struct PreparedClientControl {
    peer_addr: SocketAddr,
    peer_id: String,
    punch_socket: Arc<UdpSocket>,
    endpoint: Endpoint,
}

struct HostRelayRuntime {
    session_id: String,
    cancel: CancellationToken,
    runtime: WssRelayRuntime,
}

struct HostE4mcRuntime {
    cancel: CancellationToken,
    task: JoinHandle<()>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelEstablishedEvent {
    peer_addr: String,
    minecraft_addr: String,
    transport: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelFailedEvent {
    peer_addr: String,
    reason: String,
}

impl NetworkManager {
    pub fn new() -> Self {
        let stun = SignalingConfig::from_env();
        let wss_relay_config = WssRelayConfig::from_env("".into());
        let e4mc = E4mcConfig::from_env();
        let mut status = NetworkStatus {
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            ..Default::default()
        };
        status.logs.push("Minecraft P2P Connector started.".into());

        Self {
            inner: Arc::new(Inner {
                control: Mutex::new(()),
                session: Mutex::new(None),
                status: Arc::new(RwLock::new(status)),
                stun,
                wss_relay_config,
                e4mc,
            }),
        }
    }

    pub async fn get_status(&self) -> NetworkStatus {
        self.inner.status.read().await.clone()
    }

    pub fn shared_status(&self) -> Arc<RwLock<NetworkStatus>> {
        self.inner.status.clone()
    }

    pub fn e4mc_enabled_by_default(&self) -> bool {
        self.inner.e4mc.enabled_by_default
    }

    pub async fn start_hosting(
        &self,
        app: AppHandle,
        room_name: String,
        password: Option<String>,
        local_port: u16,
        enable_e4mc: bool,
    ) -> Result<String> {
        let room_name = room_name.trim().to_string();
        if room_name.is_empty() {
            return Err(anyhow!("room name must not be empty"));
        }
        if local_port == 0 {
            return Err(anyhow!("local game port must be > 0"));
        }

        let _guard = self.inner.control.lock().await;
        self.reset_session().await;

        match self
            .start_hosting_inner(app, room_name, password, local_port, enable_e4mc)
            .await
        {
            Ok(peer_addr) => Ok(peer_addr),
            Err(error) => {
                self.mark_fatal(SessionMode::Host, None, &error).await;
                Err(error)
            }
        }
    }

    pub async fn stop_hosting(&self) -> Result<()> {
        let _guard = self.inner.control.lock().await;
        self.reset_session().await;
        self.push_log("Session stopped.".into()).await;
        Ok(())
    }

    pub async fn connect_to_peer(
        &self,
        app: AppHandle,
        peer_addr: String,
        peer_id: Option<String>,
        relay_session_id: Option<String>,
    ) -> Result<()> {
        let peer_addr = peer_addr.trim().to_string();
        if peer_addr.is_empty() {
            return Err(anyhow!("peer address must not be empty"));
        }

        let peer_addr: SocketAddr = peer_addr
            .parse()
            .with_context(|| format!("invalid socket address: {peer_addr}"))?;

        let _guard = self.inner.control.lock().await;

        if self
            .punch_from_host(peer_addr, peer_id.clone(), relay_session_id.clone())
            .await?
        {
            return Ok(());
        }

        self.reset_session().await;
        self.start_client_connect(
            app,
            peer_addr,
            peer_id.unwrap_or_else(|| peer_addr.to_string()),
            relay_session_id,
        )
        .await
    }

    pub async fn prepare_client_connect(
        &self,
        peer_addr: String,
        peer_id: Option<String>,
    ) -> Result<()> {
        let peer_addr = peer_addr.trim().to_string();
        if peer_addr.is_empty() {
            return Err(anyhow!("peer address must not be empty"));
        }

        let peer_addr: SocketAddr = peer_addr
            .parse()
            .with_context(|| format!("invalid socket address: {peer_addr}"))?;
        let peer_id = peer_id.unwrap_or_else(|| peer_addr.to_string());

        let _guard = self.inner.control.lock().await;
        self.reset_session().await;

        let (prepared, udp_bind_addr, public_udp_addr) = self
            .prepare_client_control(peer_addr, peer_id.clone())
            .await?;
        let cancel = CancellationToken::new();

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::WaitingForPeer,
            udp_bind_addr: Some(udp_bind_addr.to_string()),
            public_udp_addr: Some(public_udp_addr.to_string()),
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some(
                "Client UDP endpoint prepared. Waiting for relay ack from host.".into(),
            ),
            peers: vec![PeerInfo {
                peer_id: peer_id.clone(),
                addr: peer_addr.to_string(),
                connected: false,
                ping_ms: None,
                transport: Some("direct-quic".into()),
            }],
            logs: vec![
                format!("Client bind: {udp_bind_addr}"),
                format!("Client public UDP: {public_udp_addr}"),
                format!("Client target: {peer_addr}"),
            ],
            ..Default::default()
        })
        .await;

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: Vec::new(),
            control: SessionControl::PreparedClient(prepared),
        });

        Ok(())
    }

    pub async fn commit_prepared_client_connect(
        &self,
        app: AppHandle,
        relay_session_id: Option<String>,
    ) -> Result<()> {
        let _guard = self.inner.control.lock().await;
        let mut session = self.inner.session.lock().await;
        let Some(runtime) = session.take() else {
            return Err(anyhow!("подготовленной клиентской сессии нет"));
        };

        let SessionRuntime {
            cancel,
            tasks,
            control,
        } = runtime;
        let reconnect_cancel = cancel.clone();
        let SessionControl::PreparedClient(prepared) = control else {
            for task in tasks {
                task.abort();
            }
            *session = Some(SessionRuntime {
                cancel,
                tasks: Vec::new(),
                control,
            });
            return Err(anyhow!("клиентский endpoint не подготовлен"));
        };
        for task in tasks {
            task.abort();
        }

        let peer_addr = prepared.peer_addr;
        let peer_id = prepared.peer_id.clone();
        let task = self.spawn_client_connect_task(
            app,
            prepared.punch_socket,
            prepared.endpoint,
            peer_addr,
            prepared.peer_id,
            relay_session_id,
            reconnect_cancel,
        );

        *session = Some(SessionRuntime {
            cancel,
            tasks: vec![task],
            control: SessionControl::Client(ClientControl { peer_addr }),
        });
        drop(session);

        self.mutate_status(|status| {
            status.mode = SessionMode::Client;
            status.state = ConnectionState::Connecting;
            status.note = Some(format!(
                "Handshake подтвержден хостом {peer_id}. Пробую direct QUIC и fallback."
            ));
        })
        .await;

        Ok(())
    }

    pub async fn kick_peer(&self, peer_id: String) -> Result<()> {
        let _guard = self.inner.control.lock().await;

        let live_connections = {
            let session = self.inner.session.lock().await;
            let runtime = session
                .as_ref()
                .ok_or_else(|| anyhow!("Р°РєС‚РёРІРЅРѕР№ СЃРµСЃСЃРёРё РЅРµС‚"))?;

            let SessionControl::Host(host) = &runtime.control else {
                return Err(anyhow!("РІС‹РіРЅР°С‚СЊ РёРіСЂРѕРєР° РјРѕР¶РЅРѕ С‚РѕР»СЊРєРѕ РёР· СЂРµР¶РёРјР° С…РѕСЃС‚Р°"));
            };

            host.live_connections.clone()
        };

        let connection = live_connections.lock().await.remove(&peer_id);
        let Some(connection) = connection else {
            return Err(anyhow!("РёРіСЂРѕРє {peer_id} РЅРµ РЅР°Р№РґРµРЅ СЃСЂРµРґРё Р°РєС‚РёРІРЅС‹С… РїРѕРґРєР»СЋС‡РµРЅРёР№"));
        };

        connection.close(VarInt::from_u32(1), b"kicked-by-host");
        self.mark_peer_disconnected(&peer_id).await;
        self.push_log(format!("РРіСЂРѕРє {peer_id} РѕС‚РєР»СЋС‡С‘РЅ С…РѕСЃС‚РѕРј."))
            .await;
        Ok(())
    }

    async fn start_hosting_inner(
        &self,
        app: AppHandle,
        room_name: String,
        password: Option<String>,
        local_port: u16,
        enable_e4mc: bool,
    ) -> Result<String> {
        let peer_id = Uuid::new_v4().to_string();
        let expected_peers = Arc::new(RwLock::new(HashMap::<SocketAddr, String>::new()));
        let live_connections = Arc::new(Mutex::new(HashMap::<String, Connection>::new()));
        let relay_sessions = Arc::new(Mutex::new(HashMap::<String, HostRelayRuntime>::new()));
        let has_password = password.is_some();
        let e4mc_runtime = if enable_e4mc {
            Some(self.spawn_e4mc_host_runtime(app.clone(), local_port))
        } else {
            None
        };

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Host,
            state: ConnectionState::Starting,
            room_code: Some(room_name.clone()),
            local_game_port: Some(local_port),
            password_protected: has_password,
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Starting host endpoint and detecting local Minecraft version.".into()),
            logs: vec![format!("Host starting: {room_name}")],
            ..Default::default()
        })
        .await;

        let minecraft_version = match minecraft::detect_local_version(local_port).await {
            Ok(version) => Some(version),
            Err(error) => {
                self.push_log(format!(
                    "Failed to detect Minecraft version on 127.0.0.1:{local_port}: {error:#}"
                ))
                .await;
                None
            }
        };

        let (udp_socket, punch_socket, udp_bind_addr) = Self::bind_shared_udp_socket()?;
        let public_udp_addr = discover_public_addr(punch_socket.clone(), &self.inner.stun).await?;
        let (server_config, _) = build_server_config()?;
        let endpoint = Endpoint::new(
            EndpointConfig::default(),
            Some(server_config),
            udp_socket,
            Arc::new(quinn::TokioRuntime),
        )
        .context("failed to create host QUIC endpoint")?;

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Host,
            state: ConnectionState::Hosting,
            room_code: Some(room_name.clone()),
            udp_bind_addr: Some(udp_bind_addr.to_string()),
            public_udp_addr: Some(public_udp_addr.to_string()),
            local_game_port: Some(local_port),
            minecraft_version: minecraft_version.clone(),
            password_protected: has_password,
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some(format!(
                "Host active. Room: {room_name}. Local port: {local_port}. Version: {}.",
                minecraft_version
                    .clone()
                    .unwrap_or_else(|| "Unknown".into())
            )),
            logs: vec![
                format!("Public UDP address: {public_udp_addr}"),
                format!("Local bind: {udp_bind_addr}"),
                format!("Host forwards to {}", proxy::minecraft_local_addr(local_port)),
            ],
            ..Default::default()
        })
        .await;

        let cancel = CancellationToken::new();
        let accept_task = self.spawn_host_accept_loop(
            app,
            endpoint,
            expected_peers.clone(),
            live_connections.clone(),
            relay_sessions.clone(),
            local_port,
            cancel.clone(),
        );

        let upnp_mapping = self.start_upnp_mapping(local_port).await;

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: vec![accept_task],
            control: SessionControl::Host(HostControl {
                punch_socket,
                room_name,
                peer_id,
                local_game_port: local_port,
                expected_peers,
                live_connections,
                relay_sessions,
                e4mc_runtime,
                upnp_mapping,
            }),
        });

        Ok(public_udp_addr.to_string())
    }

    async fn punch_from_host(
        &self,
        peer_addr: SocketAddr,
        announced_peer_id: Option<String>,
        relay_session_id: Option<String>,
    ) -> Result<bool> {
        let session = self.inner.session.lock().await;
        let Some(runtime) = session.as_ref() else {
            return Ok(false);
        };

        let SessionControl::Host(host) = &runtime.control else {
            return Ok(false);
        };

        let socket = host.punch_socket.clone();
        let cancel = runtime.cancel.clone();
        let room_name = host.room_name.clone();
        let peer_id = host.peer_id.clone();
        let local_game_port = host.local_game_port;
        let expected_peers = host.expected_peers.clone();
        let relay_sessions = host.relay_sessions.clone();
        drop(session);

        let display_peer = announced_peer_id
            .clone()
            .unwrap_or_else(|| peer_addr.to_string());
        if let Some(peer_id) = announced_peer_id {
            expected_peers.write().await.insert(peer_addr, peer_id);
        }

        self.mutate_status(|status| {
            status.state = ConnectionState::Punching;
            status.note = Some(format!(
                "Punching UDP to client {display_peer}. РРіСЂР° СЃР»СѓС€Р°РµС‚СЃСЏ РЅР° 127.0.0.1:{local_game_port}."
            ));
        })
        .await;
        self.upsert_peer(
            display_peer.clone(),
            peer_addr,
            false,
            None,
            Some("direct-quic".into()),
        )
            .await;
        self.push_log(format!("Host punch -> {display_peer} ({peer_addr})"))
            .await;

        if let Some(session_id) = relay_session_id {
            self.start_or_replace_host_relay(
                relay_sessions,
                display_peer.clone(),
                session_id,
                local_game_port,
            )
            .await;
        }

        tokio::spawn(async move {
            let _ = punch_remote(socket, peer_addr, &room_name, &peer_id, cancel).await;
        });

        Ok(true)
    }

    async fn start_client_connect(
        &self,
        app: AppHandle,
        peer_addr: SocketAddr,
        peer_id: String,
        relay_session_id: Option<String>,
    ) -> Result<()> {
        let (prepared, udp_bind_addr, public_udp_addr) = self
            .prepare_client_control(peer_addr, peer_id.clone())
            .await?;
        let cancel = CancellationToken::new();

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::WaitingForPeer,
            udp_bind_addr: Some(udp_bind_addr.to_string()),
            public_udp_addr: Some(public_udp_addr.to_string()),
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Client ready. Sending handshake and waiting for host.".into()),
            peers: vec![PeerInfo {
                peer_id: peer_id.clone(),
                addr: peer_addr.to_string(),
                connected: false,
                ping_ms: None,
                transport: Some("direct-quic".into()),
            }],
            logs: vec![
                format!("Client bind: {udp_bind_addr}"),
                format!("Client public UDP: {public_udp_addr}"),
                format!("Client target: {peer_addr}"),
            ],
            ..Default::default()
        })
        .await;

        let task = self.spawn_client_connect_task(
            app,
            prepared.punch_socket,
            prepared.endpoint,
            peer_addr,
            prepared.peer_id,
            relay_session_id,
            cancel.clone(),
        );

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: vec![task],
            control: SessionControl::Client(ClientControl { peer_addr }),
        });

        Ok(())
    }

    async fn prepare_client_control(
        &self,
        peer_addr: SocketAddr,
        peer_id: String,
    ) -> Result<(PreparedClientControl, SocketAddr, SocketAddr)> {
        let (udp_socket, punch_socket, udp_bind_addr) = Self::bind_shared_udp_socket()?;
        let public_udp_addr = discover_public_addr(punch_socket.clone(), &self.inner.stun).await?;

        let mut endpoint = Endpoint::new(
            EndpointConfig::default(),
            None,
            udp_socket,
            Arc::new(quinn::TokioRuntime),
        )
        .context("РЅРµ СѓРґР°Р»РѕСЃСЊ СЃРѕР·РґР°С‚СЊ client QUIC endpoint")?;
        endpoint.set_default_client_config(build_insecure_client_config()?);

        Ok((
            PreparedClientControl {
                peer_addr,
                peer_id,
                punch_socket,
                endpoint,
            },
            udp_bind_addr,
            public_udp_addr,
        ))
    }

    fn spawn_client_connect_task(
        &self,
        app: AppHandle,
        punch_socket: Arc<UdpSocket>,
        endpoint: Endpoint,
        peer_addr: SocketAddr,
        peer_id: String,
        relay_session_id: Option<String>,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            if let Err(error) = manager
                .run_client_connect_flow(
                    app.clone(),
                    punch_socket,
                    endpoint,
                    peer_addr,
                    peer_id.clone(),
                    relay_session_id,
                    cancel.clone(),
                )
                .await
            {
                if !cancel.is_cancelled() {
                    let _ = app.emit(
                        "tunnel_failed",
                        TunnelFailedEvent {
                            peer_addr: peer_addr.to_string(),
                            reason: "РќРµ СѓРґР°Р»РѕСЃСЊ РїСЂРѕР±РёС‚СЊ NAT Рё СѓСЃС‚Р°РЅРѕРІРёС‚СЊ С‚СѓРЅРЅРµР»СЊ.".into(),
                        },
                    );
                    manager.mark_fatal(SessionMode::Client, None, &error).await;
                }
            }
        })
    }

    async fn run_client_connect_flow(
        &self,
        app: AppHandle,
        punch_socket: Arc<UdpSocket>,
        endpoint: Endpoint,
        peer_addr: SocketAddr,
        peer_id: String,
        relay_session_id: Option<String>,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.mutate_status(|status| {
            status.mode = SessionMode::Client;
            status.state = ConnectionState::Starting;
            status.signaling_server = ABLY_SIGNAL_LABEL.into();
            status.note = Some("Хост подтвердил handshake. Поднимаю direct QUIC и fallback.".into());
        })
        .await;


        let punch_handle = tokio::spawn({
            let socket = punch_socket.clone();
            let cancel = cancel.clone();
            let room = "minecraft-p2p-connector".to_string();
            let peer = peer_id.clone();
            async move {
                let _ = punch_remote(socket, peer_addr, &room, &peer, cancel).await;
            }
        });

        tokio::select! {
            _ = cancel.cancelled() => return Err(anyhow!("РїРѕРґРєР»СЋС‡РµРЅРёРµ РѕС‚РјРµРЅРµРЅРѕ")),
            _ = tokio::time::sleep(Duration::from_millis(HOST_PUNCH_GRACE_MS)) => {}
        }

        let connection = match self
            .connect_with_retries(&endpoint, peer_addr, cancel.clone())
            .await
        {
            Ok(connection) => {
                punch_handle.abort();
                connection
            }
            Err(direct_error) => {
                punch_handle.abort();
                self.push_log(format!(
                    "Direct QUIC path failed for {peer_addr}: {direct_error:#}"
                ))
                .await;

                if let Some(session_id) = relay_session_id {
                    self.push_log(format!(
                        "Switching to relay fallback for session {session_id}."
                    ))
                    .await;
                    return self
                        .run_relay_client_tunnel(app, peer_addr, peer_id, session_id, cancel)
                        .await
                        .map_err(|relay_error| {
                            anyhow!(
                                "direct tunnel failed: {direct_error:#}\nrelay fallback failed: {relay_error:#}"
                            )
                        });
                }

                return Err(direct_error);
            }
        };

        let local_listener = TcpListener::bind(proxy::MINECRAFT_LOCAL_ADDR)
            .await
            .with_context(|| {
                format!(
                    "РЅРµ СѓРґР°Р»РѕСЃСЊ РѕС‚РєСЂС‹С‚СЊ Р»РѕРєР°Р»СЊРЅС‹Р№ РїСЂРѕРєСЃРё РЅР° {}. РџРѕСЂС‚ СѓР¶Рµ Р·Р°РЅСЏС‚",
                    proxy::MINECRAFT_LOCAL_ADDR
                )
            })?;

        self.mutate_status(|status| {
            status.state = ConnectionState::Connected;
            status.transport_path = Some("direct-quic".into());
            status.note = Some(
                "Connection established. Connect in Minecraft to localhost:25565.".into(),
            );
            status.peers = vec![PeerInfo {
                peer_id: peer_id.clone(),
                addr: peer_addr.to_string(),
                connected: true,
                ping_ms: Some(connection.rtt().as_millis() as u64),
                transport: Some("direct-quic".into()),
            }];
        })
        .await;
        self.push_log("Р›РѕРєР°Р»СЊРЅС‹Р№ proxy РЅР° 127.0.0.1:25565 РїРѕРґРЅСЏС‚.".into())
            .await;
        let _ = app.emit(
            "tunnel_established",
            TunnelEstablishedEvent {
                peer_addr: peer_addr.to_string(),
                minecraft_addr: proxy::MINECRAFT_LOCAL_ADDR.into(),
                transport: "direct-quic".into(),
            },
        );

                        let _nethernet = super::nethernet_broadcaster::NetherNetBroadcaster::start(
            format!("P2P {}", peer_id),
            19132,
            cancel.clone(),
        ).await;

        let _broadcaster = super::bedrock_broadcaster::BedrockBroadcaster::start(
            format!("P2P {}", peer_id),
            19132,
            cancel.clone(),
        ).await;

        let proxy_task =
            self.spawn_client_proxy_loop(local_listener, connection.clone(), cancel.clone());
        let ping_task = self.spawn_ping_loop(connection.clone(), peer_id.clone(), cancel.clone(), app.clone());
        let close_task = self.spawn_client_close_loop(connection, peer_id, cancel.clone());

        tokio::select! {
            _ = cancel.cancelled() => {}
            _ = async {
                let _ = tokio::join!(proxy_task, ping_task, close_task);
            } => {}
        }

        Ok(())
    }

    fn spawn_host_accept_loop(
        &self,
        app: AppHandle,
        endpoint: Endpoint,
        expected_peers: Arc<RwLock<HashMap<SocketAddr, String>>>,
        live_connections: Arc<Mutex<HashMap<String, Connection>>>,
        relay_sessions: Arc<Mutex<HashMap<String, HostRelayRuntime>>>,
        local_game_port: u16,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                let incoming = tokio::select! {
                    _ = cancel.cancelled() => break,
                    incoming = endpoint.accept() => incoming,
                };

                let Some(incoming) = incoming else {
                    break;
                };

                match incoming.await {
                    Ok(connection) => {
                        let remote = connection.remote_address();
                        let peer_id = expected_peers
                            .write()
                            .await
                            .remove(&remote)
                            .unwrap_or_else(|| remote.to_string());

                        live_connections
                            .lock()
                            .await
                            .insert(peer_id.clone(), connection.clone());
                        manager
                            .cancel_host_relay_for_peer(relay_sessions.clone(), &peer_id)
                            .await;
                        manager
                            .upsert_peer(
                                peer_id.clone(),
                                remote,
                                true,
                                Some(connection.rtt().as_millis() as u64),
                                Some("direct-quic".into()),
                            )
                            .await;
                        manager
                            .push_log(format!("Host РїСЂРёРЅСЏР» peer {peer_id} ({remote})"))
                            .await;

                        let connection_cancel = cancel.clone();
                        let connection_manager = manager.clone();
                        let live_connections = live_connections.clone();
                        let app_inner = app.clone();
                        tokio::spawn(async move {
                            connection_manager
                                .handle_host_connection(
                                    connection,
                                    peer_id,
                                    live_connections,
                                    local_game_port,
                                    connection_cancel,
                                    app_inner,
                                )
                                .await;
                        });
                    }
                    Err(error) => {
                        if !cancel.is_cancelled() {
                            manager
                                .set_nonfatal(format!("host accept failed: {error:#}"))
                                .await;
                        }
                    }
                }
            }
        })
    }

    fn spawn_client_proxy_loop(
        &self,
        listener: TcpListener,
        connection: Connection,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                let incoming = tokio::select! {
                    _ = cancel.cancelled() => break,
                    incoming = listener.accept() => incoming,
                };

                match incoming {
                    Ok((tcp_stream, _)) => {
                        let conn = connection.clone();
                        let manager = manager.clone();
                        tokio::spawn(async move {
                            if let Err(error) =
                                NetworkManager::handle_client_proxy_connection(conn, tcp_stream)
                                    .await
                            {
                                manager
                                    .set_nonfatal(format!(
                                        "Р»РѕРєР°Р»СЊРЅС‹Р№ TCP->QUIC proxy Р·Р°РІРµСЂС€РёР»СЃСЏ РѕС€РёР±РєРѕР№: {error:#}"
                                    ))
                                    .await;
                                tracing::warn!("client proxy stream failed: {error:#}");
                            }
                        });
                    }
                    Err(error) => {
                        if !cancel.is_cancelled() {
                            manager
                                .set_nonfatal(format!("local proxy listener failed: {error:#}"))
                                .await;
                        }
                        break;
                    }
                }
            }
        })
    }

    fn spawn_ping_loop(
        &self,
        connection: Connection,
        peer_id: String,
        cancel: CancellationToken,
        app: AppHandle,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                }

                let rtt = connection.rtt().as_millis() as u64;
                manager.update_peer_ping(&peer_id, rtt).await;

                // Connection Health Stats
                let stats = connection.stats();
                let _ = app.emit("peer-health", serde_json::json!({
                    "peerId": peer_id,
                    "pingMs": rtt,
                    "packetLoss": stats.path.lost_packets,
                    "sentPackets": stats.path.sent_packets,
                }));

                if connection.close_reason().is_some() {
                    break;
                }
            }
        })
    }

    fn spawn_client_close_loop(
        &self,
        connection: Connection,
        peer_id: String,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            let error = connection.closed().await;
            if !cancel.is_cancelled() {
                manager.mark_peer_disconnected(&peer_id).await;
                manager
                    .mark_fatal(
                        SessionMode::Client,
                        None,
                        &anyhow!("QUIC connection closed: {error}"),
                    )
                    .await;
            }
        })
    }

    async fn handle_host_connection(
        &self,
        connection: Connection,
        peer_id: String,
        live_connections: Arc<Mutex<HashMap<String, Connection>>>,
        local_game_port: u16,
        cancel: CancellationToken,
        app: AppHandle,
    ) {
        let ping_task = self.spawn_ping_loop(connection.clone(), peer_id.clone(), cancel.clone(), app.clone());

        loop {
            let stream = tokio::select! {
                _ = cancel.cancelled() => break,
                stream = connection.accept_bi() => stream,
            };

            match stream {
                Ok((send, recv)) => {
                    tokio::spawn(async move {
                        if let Err(error) =
                            proxy::bridge_quic_to_local_minecraft(send, recv, local_game_port).await
                        {
                            tracing::warn!("host stream proxy failed: {error:#}");
                        }
                    });
                }
                Err(quinn::ConnectionError::ApplicationClosed { .. }) => break,
                Err(error) => {
                    if !cancel.is_cancelled() {
                        self.set_nonfatal(format!("peer stream failed: {error:#}"))
                            .await;
                    }
                    break;
                }
            }
        }

        live_connections.lock().await.remove(&peer_id);
        ping_task.abort();
        self.mark_peer_disconnected(&peer_id).await;
    }

    async fn handle_client_proxy_connection(
        connection: Connection,
        tcp_stream: TcpStream,
    ) -> Result<()> {
        let mut last_error = None;
        let mut opened_stream = None;

        for attempt in 1..=3 {
            match timeout(Duration::from_secs(2), connection.open_bi()).await {
                Ok(Ok(stream)) => {
                    opened_stream = Some(stream);
                    break;
                }
                Ok(Err(error)) => {
                    tracing::warn!("open_bi attempt {attempt}/3 failed: {error:#}");
                    last_error = Some(anyhow!(error));
                }
                Err(_) => {
                    tracing::warn!("open_bi attempt {attempt}/3 timed out");
                    last_error = Some(anyhow!("open_bi timed out"));
                }
            }
        }

        let (send, recv) = opened_stream.ok_or_else(|| {
            last_error.unwrap_or_else(|| anyhow!("РЅРµ СѓРґР°Р»РѕСЃСЊ РѕС‚РєСЂС‹С‚СЊ QUIC stream РґРѕ С…РѕСЃС‚Р°"))
        })?;

        proxy::bridge_client_tcp_to_quic(tcp_stream, send, recv).await
    }

    async fn run_relay_client_tunnel(
        &self,
        app: AppHandle,
        peer_addr: SocketAddr,
        peer_id: String,
        session_id: String,
        cancel: CancellationToken,
    ) -> Result<()> {
        self.mutate_status(|status| {
            status.state = ConnectionState::Connecting;
            status.transport_path = Some("wss-relay".into());
            status.note = Some(
                "Direct UDP tunnel failed. Falling back to secure WSS tunnel (РїРѕСЂС‚ 443)."
                    .into(),
            );
        })
        .await;

        let mut relay_config = self.inner.wss_relay_config.clone();
        relay_config.session_id = session_id.clone();
        
        let runtime = wss_relay::start_client_runtime(
            relay_config,
            cancel.clone(),
        )
        .await
        .with_context(|| format!("failed to start WSS relay client session {session_id}"))?;

        self.mutate_status(|status| {
            status.state = ConnectionState::Connected;
            status.transport_path = Some("wss-relay".into());
            status.note = Some(
                "РџРѕР»РЅР°СЏ РјР°СЃРєРёСЂРѕРІРєР°. РЎРѕРµРґРёРЅРµРЅРёРµ СѓСЃС‚Р°РЅРѕРІР»РµРЅРѕ via РЎРµРєСЂРµС‚РЅС‹Р№ РўСѓРЅРЅРµР»СЊ 443. РџРѕРґРєР»СЋС‡Р°Р№С‚РµСЃСЊ Рє localhost:25565."
                    .into(),
            );
            status.peers = vec![PeerInfo {
                peer_id: peer_id.clone(),
                addr: peer_addr.to_string(),
                connected: true,
                ping_ms: None,
                transport: Some("wss-relay".into()),
            }];
        })
        .await;
        self.push_log(format!(
            "Relay fallback ready for {peer_addr} via session {session_id}."
        ))
        .await;
        let _ = app.emit(
            "tunnel_established",
            TunnelEstablishedEvent {
                peer_addr: peer_addr.to_string(),
                minecraft_addr: proxy::MINECRAFT_LOCAL_ADDR.into(),
                transport: "wss-relay".into(),
            },
        );

        runtime.wait().await
    }

    async fn start_or_replace_host_relay(
        &self,
        relay_sessions: Arc<Mutex<HashMap<String, HostRelayRuntime>>>,
        peer_id: String,
        session_id: String,
        local_game_port: u16,
    ) {
        let mut relay_config = self.inner.wss_relay_config.clone();
        relay_config.session_id = session_id.clone();
        let cancel = CancellationToken::new();

        let runtime_result = wss_relay::start_host_runtime(relay_config, local_game_port, cancel.clone()).await;

        match runtime_result {
            Ok(runtime) => {
                let replaced = relay_sessions.lock().await.insert(
                    peer_id.clone(),
                    HostRelayRuntime {
                        session_id: session_id.clone(),
                        cancel,
                        runtime,
                    },
                );

                if let Some(previous) = replaced {
                    previous.cancel.cancel();
                    previous.runtime.abort();
                }

                self.push_log(format!(
                    "Host armed WSS (443) relay fallback for {peer_id} via session {session_id}."
                ))
                .await;
            }
            Err(error) => {
                self.set_nonfatal(format!(
                    "failed to bootstrap host WSS relay session {session_id}: {error:#}"
                ))
                .await;
            }
        }
    }

    async fn cancel_host_relay_for_peer(
        &self,
        relay_sessions: Arc<Mutex<HashMap<String, HostRelayRuntime>>>,
        peer_id: &str,
    ) {
        if let Some(runtime) = relay_sessions.lock().await.remove(peer_id) {
            runtime.cancel.cancel();
            runtime.runtime.abort();
            self.push_log(format!(
                "Direct QUIC won for {peer_id}; WSS relay session {} cancelled.",
                runtime.session_id
            ))
            .await;
        }
    }

    async fn connect_with_retries(
        &self,
        endpoint: &Endpoint,
        peer_addr: SocketAddr,
        cancel: CancellationToken,
    ) -> Result<Connection> {
        let mut last_error = None;

        for attempt in 1..=CLIENT_CONNECT_RETRY_ATTEMPTS {
            if cancel.is_cancelled() {
                return Err(anyhow!("РїРѕРґРєР»СЋС‡РµРЅРёРµ РѕС‚РјРµРЅРµРЅРѕ"));
            }

            self.mutate_status(|status| {
                status.state = ConnectionState::Connecting;
                status.note = Some(format!(
                    "QUIC handshake, РїРѕРїС‹С‚РєР° {attempt}/{CLIENT_CONNECT_RETRY_ATTEMPTS}. Р–РґСѓ РѕС‚РІРµС‚РЅС‹Р№ NAT punch."
                ));
            })
            .await;

            let connect = endpoint
                .connect(peer_addr, "localhost")
                .context("РЅРµ СѓРґР°Р»РѕСЃСЊ Р·Р°РїСѓСЃС‚РёС‚СЊ QUIC connect")?;

            match timeout(Duration::from_millis(CLIENT_CONNECT_TIMEOUT_MS), connect).await {
                Ok(Ok(connection)) => return Ok(connection),
                Ok(Err(error)) => last_error = Some(anyhow!(error)),
                Err(_) => last_error = Some(anyhow!("QUIC handshake timed out")),
            }

            tokio::time::sleep(Duration::from_millis(CLIENT_CONNECT_DELAY_MS)).await;
        }

        Err(last_error.unwrap_or_else(|| anyhow!("РЅРµ СѓРґР°Р»РѕСЃСЊ СѓСЃС‚Р°РЅРѕРІРёС‚СЊ QUIC session")))
    }

    fn bind_shared_udp_socket() -> Result<(std::net::UdpSocket, Arc<UdpSocket>, SocketAddr)> {
        let socket = std::net::UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        let addr = socket.local_addr()?;
        let tokio_socket = Arc::new(UdpSocket::from_std(socket.try_clone()?)?);
        Ok((socket, tokio_socket, addr))
    }

    async fn reset_session(&self) {
        let mut session = self.inner.session.lock().await;
        if let Some(runtime) = session.take() {
            runtime.cancel.cancel();

            match runtime.control {
                SessionControl::Host(host) => {
                    let mut live_connections = host.live_connections.lock().await;
                    for (_, connection) in live_connections.drain() {
                        connection.close(VarInt::from_u32(0), b"session-reset");
                    }
                    if let Some(runtime) = host.e4mc_runtime {
                        runtime.cancel.cancel();
                        runtime.task.abort();
                    }
                    let mut relay_sessions = host.relay_sessions.lock().await;
                    for (_, runtime) in relay_sessions.drain() {
                        runtime.cancel.cancel();
                        runtime.runtime.abort();
                    }
                }
                SessionControl::PreparedClient(client) => {
                    self.push_log(format!(
                        "Подготовленная клиентская сессия с {} очищена.",
                        client.peer_addr
                    ))
                    .await;
                }
                SessionControl::Client(client) => {
                    self.push_log(format!("РљР»РёРµРЅС‚СЃРєР°СЏ СЃРµСЃСЃРёСЏ СЃ {} РѕС‡РёС‰РµРЅР°.", client.peer_addr))
                        .await;
                }
            }

            for task in runtime.tasks {
                task.abort();
            }
        }
        drop(session);

        let mut status = NetworkStatus {
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            ..Default::default()
        };
        status.logs.push("Session cleared.".into());
        self.overwrite_status(status).await;
    }

    fn spawn_e4mc_host_runtime(&self, app: AppHandle, local_game_port: u16) -> HostE4mcRuntime {
        let manager = self.clone();
        let config = self.inner.e4mc.clone();
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();

        let task = tokio::spawn(async move {
            loop {
                if task_cancel.is_cancelled() { break; }
                manager
                    .push_log(format!(
                        "Starting e4mc public fallback for local Minecraft 127.0.0.1:{local_game_port}."
                    ))
                    .await;

                match e4mc::start_host_runtime(config.clone(), local_game_port, task_cancel.clone()).await {
                    Ok(runtime) => {
                        let domain = runtime.domain.clone();
                        manager
                            .mutate_status(|status| {
                                status.e4mc_domain = Some(domain.clone());
                                status.e4mc_verified = false;
                                if status.transport_path.is_none() {
                                    status.transport_path = Some("e4mc-public".into());
                                }
                                status.note = Some(format!(
                                    "Host is active. Direct transport remains primary, e4mc assigned {domain} and is being verified."
                                ));
                            })
                            .await;
                        manager
                            .push_log(format!("e4mc public domain assigned: {domain}"))
                            .await;

                        let verification = manager
                            .verify_e4mc_public_domain(&domain, local_game_port, task_cancel.clone())
                            .await;

                        match verification {
                            Ok(probe) => {
                                manager
                                    .mutate_status(|status| {
                                        status.e4mc_domain = Some(domain.clone());
                                        status.e4mc_verified = true;
                                        status.public_join_address = Some(domain.clone());
                                        if status.transport_path.is_none() {
                                            status.transport_path = Some("e4mc-public".into());
                                        }
                                        status.note = Some(format!(
                                            "Host is active. Direct transport remains primary, verified e4mc fallback is ready at {domain}."
                                        ));
                                    })
                                    .await;
                                manager
                                    .push_log(format!(
                                        "e4mc public domain verified: {domain} -> Minecraft {} ({}/{})",
                                        probe
                                            .version
                                            .clone()
                                            .unwrap_or_else(|| "unknown".into()),
                                        probe.online_players,
                                        probe.max_players
                                    ))
                                    .await;
                                let _ = app.emit(
                                    "e4mc_domain_ready",
                                    serde_json::json!({ "domain": domain, "verified": true }),
                                );
                            }
                            Err(error) => {
                                manager
                                    .mutate_status(|status| {
                                        status.e4mc_domain = Some(domain.clone());
                                        status.e4mc_verified = false;
                                        status.public_join_address = None;
                                        status.note = Some(format!(
                                            "Host is active, but e4mc domain {domain} failed verification. Direct transport remains available."
                                        ));
                                    })
                                    .await;
                                manager
                                    .push_log(format!(
                                        "e4mc verification failed for {domain}; public link disabled: {error:#}"
                                    ))
                                    .await;
                                let _ = app.emit(
                                    "e4mc_domain_ready",
                                    serde_json::json!({
                                        "domain": domain,
                                        "verified": false,
                                        "error": format!("{error:#}")
                                    }),
                                );
                            }
                        }

                        if let Err(error) = runtime.wait().await {
                            if !task_cancel.is_cancelled() {
                                manager
                                    .push_log(format!("e4mc session terminated: {error:#}. Restarting in 5s..."))
                                    .await;
                            }
                        }
                    }
                    Err(error) => {
                        if !task_cancel.is_cancelled() {
                            manager
                                .push_log(format!("e4mc fallback unavailable: {error:#}. Retrying in 10s..."))
                                .await;
                        }
                    }
                }
                
                if task_cancel.is_cancelled() { break; }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        });

        HostE4mcRuntime { cancel, task }
    }

    async fn verify_e4mc_public_domain(
        &self,
        domain: &str,
        local_game_port: u16,
        cancel: CancellationToken,
    ) -> Result<ExternalServerProbe> {
        let mut last_error = None;

        for attempt in 1..=6 {
            if cancel.is_cancelled() {
                return Err(anyhow!("e4mc verification cancelled"));
            }

            self.push_log(format!(
                "Verifying e4mc public domain {domain} (attempt {attempt}/6) via public Minecraft port 25565 -> local {local_game_port}."
            ))
            .await;

            match minecraft::probe_external_server(domain.to_string(), 25565).await {
                Ok(probe) => return Ok(probe),
                Err(error) => {
                    last_error = Some(error);
                    if attempt < 6 {
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("e4mc verification failed without a concrete error")))
    }

    async fn overwrite_status(&self, status: NetworkStatus) {
        *self.inner.status.write().await = status;
    }

    async fn mutate_status<F>(&self, update: F)
    where
        F: FnOnce(&mut NetworkStatus),
    {
        let mut status = self.inner.status.write().await;
        update(&mut status);
        status.peer_count = status.peers.iter().filter(|peer| peer.connected).count();
    }

    async fn push_log(&self, entry: String) {
        self.mutate_status(|status| {
            status.logs.insert(0, entry);
            if status.logs.len() > 64 {
                status.logs.truncate(64);
            }
        })
        .await;
    }

    async fn upsert_peer(
        &self,
        peer_id: String,
        addr: SocketAddr,
        connected: bool,
        ping_ms: Option<u64>,
        transport: Option<String>,
    ) {
        self.mutate_status(|status| {
            if let Some(peer) = status.peers.iter_mut().find(|peer| peer.peer_id == peer_id) {
                peer.addr = addr.to_string();
                peer.connected = connected;
                peer.ping_ms = ping_ms;
                peer.transport = transport.clone().or_else(|| peer.transport.clone());
            } else {
                status.peers.push(PeerInfo {
                    peer_id,
                    addr: addr.to_string(),
                    connected,
                    ping_ms,
                    transport,
                });
            }

            if status.mode == SessionMode::Host {
                status.state = if status.peers.iter().any(|peer| peer.connected) {
                    ConnectionState::Connected
                } else {
                    ConnectionState::Hosting
                };
            }
        })
        .await;
    }

    async fn update_peer_ping(&self, peer_id: &str, ping_ms: u64) {
        self.mutate_status(|status| {
            if let Some(peer) = status.peers.iter_mut().find(|peer| peer.peer_id == peer_id) {
                peer.ping_ms = Some(ping_ms);
            }
        })
        .await;
    }

    async fn mark_peer_disconnected(&self, peer_id: &str) {
        self.mutate_status(|status| {
            if let Some(peer) = status.peers.iter_mut().find(|peer| peer.peer_id == peer_id) {
                peer.connected = false;
            }

            if status.mode == SessionMode::Host {
                status.state = ConnectionState::Hosting;
                status.note = Some("РРіСЂРѕРє РѕС‚РєР»СЋС‡РёР»СЃСЏ, С…РѕСЃС‚ РѕСЃС‚Р°С‘С‚СЃСЏ Р°РєС‚РёРІРЅС‹Рј.".into());
            }
        })
        .await;
    }

    async fn set_nonfatal(&self, message: String) {
        let log_message = message.clone();
        self.mutate_status(|status| {
            status.last_error = Some(message);
        })
        .await;
        self.push_log(log_message).await;
    }

    async fn mark_fatal(
        &self,
        mode: SessionMode,
        room_code: Option<String>,
        error: &anyhow::Error,
    ) {
        let formatted = format!("{error:#}");
        self.overwrite_status(NetworkStatus {
            mode,
            state: ConnectionState::Error,
            room_code,
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            last_error: Some(formatted.clone()),
            note: Some("РЎРµСЃСЃРёСЏ Р·Р°РІРµСЂС€РёР»Р°СЃСЊ СЃ РѕС€РёР±РєРѕР№.".into()),
            logs: vec![formatted],
            ..Default::default()
        })
        .await;
    }

    async fn start_upnp_mapping(&self, local_port: u16) -> Option<super::upnp::UpnpMapping> {
        match super::upnp::UpnpMapping::attempt_map(local_port, "Minecraft P2P Connector").await {
            Ok(mapping) => Some(mapping),
            Err(e) => {
                let _ = self.push_log(format!("UPnP mapping failed: {e:#}")).await;
                None
            }
        }
    }
}



