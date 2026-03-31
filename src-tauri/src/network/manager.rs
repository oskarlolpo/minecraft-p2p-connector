use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use quinn::{Connection, Endpoint, EndpointConfig};
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

use super::proxy;

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
    Client,
}

struct HostControl {
    punch_socket: Arc<UdpSocket>,
    room_name: String,
    peer_id: String,
}

impl NetworkManager {
    pub fn new() -> Self {
        let stun = SignalingConfig::from_env();
        let mut status = NetworkStatus {
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            ..Default::default()
        };
        status.logs.push("Blood Paradise Hub запущен.".into());

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
        room_name: String,
        password: Option<String>,
    ) -> Result<String> {
        let room_name = room_name.trim().to_string();
        if room_name.is_empty() {
            return Err(anyhow!("room name must not be empty"));
        }

        let _guard = self.inner.control.lock().await;
        self.reset_session().await;

        match self.start_hosting_inner(room_name, password).await {
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

    pub async fn connect_to_peer(&self, peer_addr: String) -> Result<()> {
        let peer_addr = peer_addr.trim().to_string();
        if peer_addr.is_empty() {
            return Err(anyhow!("peer address must not be empty"));
        }

        let peer_addr: SocketAddr = peer_addr
            .parse()
            .with_context(|| format!("invalid peer socket address: {peer_addr}"))?;

        let _guard = self.inner.control.lock().await;

        if self.punch_from_host(peer_addr).await? {
            return Ok(());
        }

        self.reset_session().await;
        self.start_client_connect(peer_addr).await
    }

    async fn start_hosting_inner(
        &self,
        room_name: String,
        password: Option<String>,
    ) -> Result<String> {
        let peer_id = Uuid::new_v4().to_string();
        let peer_map = Arc::new(RwLock::new(HashMap::<SocketAddr, String>::new()));

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Host,
            state: ConnectionState::Starting,
            room_code: Some(room_name.clone()),
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Поднимаю host endpoint и вычисляю внешний UDP адрес.".into()),
            logs: vec![format!("Host стартует: {room_name}")],
            ..Default::default()
        })
        .await;

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
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some(format!(
                "Host активен. Комната: {room_name}. {}",
                if password.is_some() {
                    "Лобби защищено паролем."
                } else {
                    "Лобби открыто."
                }
            )),
            logs: vec![
                format!("Публичный UDP адрес: {public_udp_addr}"),
                format!("Локальный bind: {udp_bind_addr}"),
            ],
            ..Default::default()
        })
        .await;

        let cancel = CancellationToken::new();
        let accept_task = self.spawn_host_accept_loop(endpoint, peer_map, cancel.clone());

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: vec![accept_task],
            control: SessionControl::Host(HostControl {
                punch_socket,
                room_name,
                peer_id,
            }),
        });

        Ok(public_udp_addr.to_string())
    }

    async fn punch_from_host(&self, peer_addr: SocketAddr) -> Result<bool> {
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
        drop(session);

        self.mutate_status(|status| {
            status.state = ConnectionState::Punching;
            status.note = Some(format!("Пробиваю UDP до клиента {peer_addr}."));
        })
        .await;
        self.upsert_peer(peer_addr.to_string(), peer_addr, false, None)
            .await;
        self.push_log(format!("Host punch -> {peer_addr}")).await;

        tokio::spawn(async move {
            let _ = punch_remote(socket, peer_addr, &room_name, &peer_id, cancel).await;
        });

        Ok(true)
    }

    async fn start_client_connect(&self, peer_addr: SocketAddr) -> Result<()> {
        let cancel = CancellationToken::new();
        let task = self.spawn_client_connect_flow(peer_addr, cancel.clone());

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: vec![task],
            control: SessionControl::Client,
        });

        Ok(())
    }

    fn spawn_client_connect_flow(
        &self,
        peer_addr: SocketAddr,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            if let Err(error) = manager
                .run_client_connect_flow(peer_addr, cancel.clone())
                .await
            {
                if !cancel.is_cancelled() {
                    manager.mark_fatal(SessionMode::Client, None, &error).await;
                }
            }
        })
    }

    async fn run_client_connect_flow(
        &self,
        peer_addr: SocketAddr,
        cancel: CancellationToken,
    ) -> Result<()> {
        let peer_id = peer_addr.to_string();

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::Starting,
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Подготавливаю client endpoint и узнаю внешний UDP адрес.".into()),
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
        .context("failed to create client QUIC endpoint")?;
        endpoint.set_default_client_config(build_insecure_client_config()?);

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::WaitingForPeer,
            udp_bind_addr: Some(udp_bind_addr.to_string()),
            public_udp_addr: Some(public_udp_addr.to_string()),
            signaling_server: ABLY_SIGNAL_LABEL.into(),
            note: Some("Локальный клиент готов. Отправляю handshake и жду punch от хоста.".into()),
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
            let room = "blood-paradise".to_string();
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
                    "failed to bind local proxy on {}. Minecraft or another app may already use it",
                    proxy::MINECRAFT_LOCAL_ADDR
                )
            })?;

        self.mutate_status(|status| {
            status.state = ConnectionState::Connected;
            status.note = Some("Соединение установлено. Подключайтесь в Minecraft к localhost.".into());
            status.peers = vec![PeerInfo {
                peer_id: peer_id.clone(),
                addr: peer_addr.to_string(),
                connected: true,
                ping_ms: Some(connection.rtt().as_millis() as u64),
            }];
        })
        .await;
        self.push_log("Proxy на 127.0.0.1:25565 поднят.".into()).await;

        let proxy_task = self.spawn_client_proxy_loop(local_listener, connection.clone(), cancel.clone());
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
        peer_map: Arc<RwLock<HashMap<SocketAddr, String>>>,
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
                        let peer_id = peer_map
                            .read()
                            .await
                            .get(&remote)
                            .cloned()
                            .unwrap_or_else(|| remote.to_string());

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
                        tokio::spawn(async move {
                            connection_manager
                                .handle_host_connection(connection, peer_id, connection_cancel)
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
                        tokio::spawn(async move {
                            if let Err(error) =
                                NetworkManager::handle_client_proxy_connection(conn, tcp_stream).await
                            {
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
                        if let Err(error) = proxy::bridge_quic_to_local_minecraft(send, recv).await
                        {
                            tracing::warn!("host stream proxy failed: {error:#}");
                        }
                    });
                }
                Err(quinn::ConnectionError::ApplicationClosed { .. }) => break,
                Err(error) => {
                    if !cancel.is_cancelled() {
                        self.set_nonfatal(format!("peer stream failed: {error:#}")).await;
                    }
                    break;
                }
            }
        }

        ping_task.abort();
        self.mark_peer_disconnected(&peer_id).await;
    }

    async fn handle_client_proxy_connection(
        connection: Connection,
        tcp_stream: TcpStream,
    ) -> Result<()> {
        let (send, recv) = connection
            .open_bi()
            .await
            .context("failed to open QUIC stream to host")?;
        proxy::bridge_client_tcp_to_quic(tcp_stream, send, recv).await
    }

    async fn connect_with_retries(
        &self,
        endpoint: &Endpoint,
        peer_addr: SocketAddr,
        cancel: CancellationToken,
    ) -> Result<Connection> {
        let mut last_error = None;

        for attempt in 1..=8 {
            if cancel.is_cancelled() {
                return Err(anyhow!("connection attempt cancelled"));
            }

            self.mutate_status(|status| {
                status.state = ConnectionState::Connecting;
                status.note = Some(format!("QUIC handshake, попытка {attempt}/8."));
            })
            .await;

            let connect = endpoint
                .connect(peer_addr, "localhost")
                .context("failed to start QUIC connect")?;

            match timeout(Duration::from_secs(3), connect).await {
                Ok(Ok(connection)) => return Ok(connection),
                Ok(Err(error)) => last_error = Some(anyhow!(error)),
                Err(_) => last_error = Some(anyhow!("QUIC handshake timed out")),
            }

            tokio::time::sleep(Duration::from_millis(350)).await;
        }

        Err(last_error.unwrap_or_else(|| anyhow!("unable to establish QUIC session")))
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
            if status.logs.len() > 48 {
                status.logs.truncate(48);
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
                status.note = Some("Peer отключился, host остаётся активным.".into());
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
