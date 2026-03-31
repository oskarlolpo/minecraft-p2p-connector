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
    models::{ConnectionState, NetworkStatus, PeerInfo, SessionMode},
    signaling::{discover_public_addr, punch_remote, SignalingConfig},
};

use super::{minecraft, proxy};

const ABLY_SIGNAL_LABEL: &str = "Ably Presence + Channels";

#[derive(Clone)]
pub struct NetworkManager {
    inner: Arc<Inner>,
}

struct Inner {
    control: Mutex<()>,
    session: Mutex<Option<SessionRuntime>>,
    status: RwLock<NetworkStatus>,
    stun: SignalingConfig,
}

struct SessionRuntime {
    cancel: CancellationToken,
    tasks: Vec<JoinHandle<()>>,
    control: SessionControl,
}

enum SessionControl {
    Host(HostControl),
    Client(ClientControl),
}

struct HostControl {
    punch_socket: Arc<UdpSocket>,
    room_name: String,
    peer_id: String,
    local_game_port: u16,
    expected_peers: Arc<RwLock<HashMap<SocketAddr, String>>>,
    live_connections: Arc<Mutex<HashMap<String, Connection>>>,
}

struct ClientControl {
    peer_addr: SocketAddr,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelEstablishedEvent {
    peer_addr: String,
    minecraft_addr: String,
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
        let mut status = NetworkStatus {
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            ..Default::default()
        };
        status.logs.push("Minecraft P2P Connector запущен.".into());

        Self {
            inner: Arc::new(Inner {
                control: Mutex::new(()),
                session: Mutex::new(None),
                status: RwLock::new(status),
                stun,
            }),
        }
    }

    pub async fn get_status(&self) -> NetworkStatus {
        self.inner.status.read().await.clone()
    }

    pub async fn start_hosting(
        &self,
        _app: AppHandle,
        room_name: String,
        password: Option<String>,
        local_port: u16,
    ) -> Result<String> {
        let room_name = room_name.trim().to_string();
        if room_name.is_empty() {
            return Err(anyhow!("имя комнаты не должно быть пустым"));
        }
        if local_port == 0 {
            return Err(anyhow!("порт локальной игры должен быть больше 0"));
        }

        let _guard = self.inner.control.lock().await;
        self.reset_session().await;

        match self
            .start_hosting_inner(room_name, password, local_port)
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
        self.push_log("Сессия остановлена.".into()).await;
        Ok(())
    }

    pub async fn connect_to_peer(
        &self,
        app: AppHandle,
        peer_addr: String,
        peer_id: Option<String>,
    ) -> Result<()> {
        let peer_addr = peer_addr.trim().to_string();
        if peer_addr.is_empty() {
            return Err(anyhow!("адрес peer не должен быть пустым"));
        }

        let peer_addr: SocketAddr = peer_addr
            .parse()
            .with_context(|| format!("неверный socket address: {peer_addr}"))?;

        let _guard = self.inner.control.lock().await;

        if self.punch_from_host(peer_addr, peer_id.clone()).await? {
            return Ok(());
        }

        self.reset_session().await;
        self.start_client_connect(app, peer_addr).await
    }

    pub async fn kick_peer(&self, peer_id: String) -> Result<()> {
        let _guard = self.inner.control.lock().await;

        let live_connections = {
            let session = self.inner.session.lock().await;
            let runtime = session
                .as_ref()
                .ok_or_else(|| anyhow!("активной сессии нет"))?;

            let SessionControl::Host(host) = &runtime.control else {
                return Err(anyhow!("выгнать игрока можно только из режима хоста"));
            };

            host.live_connections.clone()
        };

        let connection = live_connections.lock().await.remove(&peer_id);
        let Some(connection) = connection else {
            return Err(anyhow!("игрок {peer_id} не найден среди активных подключений"));
        };

        connection.close(VarInt::from_u32(1), b"kicked-by-host");
        self.mark_peer_disconnected(&peer_id).await;
        self.push_log(format!("Игрок {peer_id} отключён хостом."))
            .await;
        Ok(())
    }

    async fn start_hosting_inner(
        &self,
        room_name: String,
        password: Option<String>,
        local_port: u16,
    ) -> Result<String> {
        let peer_id = Uuid::new_v4().to_string();
        let expected_peers = Arc::new(RwLock::new(HashMap::<SocketAddr, String>::new()));
        let live_connections = Arc::new(Mutex::new(HashMap::<String, Connection>::new()));
        let has_password = password.is_some();

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Host,
            state: ConnectionState::Starting,
            room_code: Some(room_name.clone()),
            local_game_port: Some(local_port),
            password_protected: has_password,
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Поднимаю host endpoint и запрашиваю локальную версию Minecraft.".into()),
            logs: vec![format!("Host стартует: {room_name}")],
            ..Default::default()
        })
        .await;

        let minecraft_version = match minecraft::detect_local_version(local_port).await {
            Ok(version) => Some(version),
            Err(error) => {
                self.push_log(format!(
                    "Не удалось определить версию Minecraft на 127.0.0.1:{local_port}: {error:#}"
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
        .context("не удалось создать host QUIC endpoint")?;

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
                "Хост активен. Комната: {room_name}. Локальный порт: {local_port}. Версия: {}.",
                minecraft_version
                    .clone()
                    .unwrap_or_else(|| "Неизвестно".into())
            )),
            logs: vec![
                format!("Публичный UDP адрес: {public_udp_addr}"),
                format!("Локальный bind: {udp_bind_addr}"),
                format!("Host forwards to {}", proxy::minecraft_local_addr(local_port)),
            ],
            ..Default::default()
        })
        .await;

        let cancel = CancellationToken::new();
        let accept_task = self.spawn_host_accept_loop(
            endpoint,
            expected_peers.clone(),
            live_connections.clone(),
            local_port,
            cancel.clone(),
        );

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
            }),
        });

        Ok(public_udp_addr.to_string())
    }

    async fn punch_from_host(
        &self,
        peer_addr: SocketAddr,
        announced_peer_id: Option<String>,
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
                "Пробиваю UDP до клиента {display_peer}. Игра слушается на 127.0.0.1:{local_game_port}."
            ));
        })
        .await;
        self.upsert_peer(display_peer.clone(), peer_addr, false, None)
            .await;
        self.push_log(format!("Host punch -> {display_peer} ({peer_addr})"))
            .await;

        tokio::spawn(async move {
            let _ = punch_remote(socket, peer_addr, &room_name, &peer_id, cancel).await;
        });

        Ok(true)
    }

    async fn start_client_connect(&self, app: AppHandle, peer_addr: SocketAddr) -> Result<()> {
        let cancel = CancellationToken::new();
        let task = self.spawn_client_connect_flow(app, peer_addr, cancel.clone());

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: vec![task],
            control: SessionControl::Client(ClientControl { peer_addr }),
        });

        Ok(())
    }

    fn spawn_client_connect_flow(
        &self,
        app: AppHandle,
        peer_addr: SocketAddr,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            if let Err(error) = manager
                .run_client_connect_flow(app.clone(), peer_addr, cancel.clone())
                .await
            {
                if !cancel.is_cancelled() {
                    let _ = app.emit(
                        "tunnel_failed",
                        TunnelFailedEvent {
                            peer_addr: peer_addr.to_string(),
                            reason: "Не удалось пробить NAT и установить туннель.".into(),
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
        peer_addr: SocketAddr,
        cancel: CancellationToken,
    ) -> Result<()> {
        let peer_id = peer_addr.to_string();

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::Starting,
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Подготавливаю клиентский endpoint и вычисляю внешний UDP адрес.".into()),
            logs: vec![format!("Client target: {peer_addr}")],
            ..Default::default()
        })
        .await;

        let (udp_socket, punch_socket, udp_bind_addr) = Self::bind_shared_udp_socket()?;
        let public_udp_addr = discover_public_addr(punch_socket.clone(), &self.inner.stun).await?;

        let mut endpoint = Endpoint::new(
            EndpointConfig::default(),
            None,
            udp_socket,
            Arc::new(quinn::TokioRuntime),
        )
        .context("не удалось создать client QUIC endpoint")?;
        endpoint.set_default_client_config(build_insecure_client_config()?);

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::WaitingForPeer,
            udp_bind_addr: Some(udp_bind_addr.to_string()),
            public_udp_addr: Some(public_udp_addr.to_string()),
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Клиент готов. Отправляю handshake и жду ответ от хоста.".into()),
            peers: vec![PeerInfo {
                peer_id: peer_id.clone(),
                addr: peer_addr.to_string(),
                connected: false,
                ping_ms: None,
            }],
            logs: vec![
                format!("Client bind: {udp_bind_addr}"),
                format!("Client public UDP: {public_udp_addr}"),
            ],
            ..Default::default()
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

        let connection = self
            .connect_with_retries(&endpoint, peer_addr, cancel.clone())
            .await?;
        punch_handle.abort();

        let local_listener = TcpListener::bind(proxy::MINECRAFT_LOCAL_ADDR)
            .await
            .with_context(|| {
                format!(
                    "не удалось открыть локальный прокси на {}. Порт уже занят",
                    proxy::MINECRAFT_LOCAL_ADDR
                )
            })?;

        self.mutate_status(|status| {
            status.state = ConnectionState::Connected;
            status.note = Some(
                "Соединение установлено. Подключайтесь в Minecraft к localhost:25565.".into(),
            );
            status.peers = vec![PeerInfo {
                peer_id: peer_id.clone(),
                addr: peer_addr.to_string(),
                connected: true,
                ping_ms: Some(connection.rtt().as_millis() as u64),
            }];
        })
        .await;
        self.push_log("Локальный proxy на 127.0.0.1:25565 поднят.".into())
            .await;
        let _ = app.emit(
            "tunnel_established",
            TunnelEstablishedEvent {
                peer_addr: peer_addr.to_string(),
                minecraft_addr: proxy::MINECRAFT_LOCAL_ADDR.into(),
            },
        );

        let proxy_task =
            self.spawn_client_proxy_loop(local_listener, connection.clone(), cancel.clone());
        let ping_task = self.spawn_ping_loop(connection.clone(), peer_id.clone(), cancel.clone());
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
        endpoint: Endpoint,
        expected_peers: Arc<RwLock<HashMap<SocketAddr, String>>>,
        live_connections: Arc<Mutex<HashMap<String, Connection>>>,
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
                            .upsert_peer(
                                peer_id.clone(),
                                remote,
                                true,
                                Some(connection.rtt().as_millis() as u64),
                            )
                            .await;
                        manager
                            .push_log(format!("Host принял peer {peer_id} ({remote})"))
                            .await;

                        let connection_cancel = cancel.clone();
                        let connection_manager = manager.clone();
                        let live_connections = live_connections.clone();
                        tokio::spawn(async move {
                            connection_manager
                                .handle_host_connection(
                                    connection,
                                    peer_id,
                                    live_connections,
                                    local_game_port,
                                    connection_cancel,
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
                                        "локальный TCP->QUIC proxy завершился ошибкой: {error:#}"
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
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {}
                }

                manager
                    .update_peer_ping(&peer_id, connection.rtt().as_millis() as u64)
                    .await;

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
    ) {
        let ping_task = self.spawn_ping_loop(connection.clone(), peer_id.clone(), cancel.clone());

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
            last_error.unwrap_or_else(|| anyhow!("не удалось открыть QUIC stream до хоста"))
        })?;

        proxy::bridge_client_tcp_to_quic(tcp_stream, send, recv).await
    }

    async fn connect_with_retries(
        &self,
        endpoint: &Endpoint,
        peer_addr: SocketAddr,
        cancel: CancellationToken,
    ) -> Result<Connection> {
        let mut last_error = None;

        for attempt in 1..=4 {
            if cancel.is_cancelled() {
                return Err(anyhow!("подключение отменено"));
            }

            self.mutate_status(|status| {
                status.state = ConnectionState::Connecting;
                status.note = Some(format!("QUIC handshake, попытка {attempt}/4."));
            })
            .await;

            let connect = endpoint
                .connect(peer_addr, "localhost")
                .context("не удалось запустить QUIC connect")?;

            match timeout(Duration::from_millis(1250), connect).await {
                Ok(Ok(connection)) => return Ok(connection),
                Ok(Err(error)) => last_error = Some(anyhow!(error)),
                Err(_) => last_error = Some(anyhow!("QUIC handshake timed out")),
            }

            tokio::time::sleep(Duration::from_millis(250)).await;
        }

        Err(last_error.unwrap_or_else(|| anyhow!("не удалось установить QUIC session")))
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
                }
                SessionControl::Client(client) => {
                    self.push_log(format!("Клиентская сессия с {} очищена.", client.peer_addr))
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
    ) {
        self.mutate_status(|status| {
            if let Some(peer) = status.peers.iter_mut().find(|peer| peer.peer_id == peer_id) {
                peer.addr = addr.to_string();
                peer.connected = connected;
                peer.ping_ms = ping_ms;
            } else {
                status.peers.push(PeerInfo {
                    peer_id,
                    addr: addr.to_string(),
                    connected,
                    ping_ms,
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
                status.note = Some("Игрок отключился, хост остаётся активным.".into());
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
            note: Some("Сессия завершилась с ошибкой.".into()),
            logs: vec![formatted],
            ..Default::default()
        })
        .await;
    }
}
