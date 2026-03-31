use std::{collections::HashMap, net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use quinn::{Connection, Endpoint, EndpointConfig};
use rumqttc::AsyncClient;
use tokio::{
    net::{TcpListener, TcpStream, UdpSocket},
    sync::{Mutex, RwLock},
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::{
    cert::{build_client_config, build_server_config},
    models::{ConnectionState, NetworkStatus, PeerInfo, SessionMode},
    signaling::{
        discover_public_addr, new_room_code, publish_broker_message, punch_remote, room_join_topic,
        room_offer_topic, subscribe_topic, BrokerConnection, BrokerMessage, SignalingConfig,
    },
};

use super::proxy;

#[derive(Clone)]
pub struct NetworkManager {
    inner: Arc<Inner>,
}

struct Inner {
    control: Mutex<()>,
    session: Mutex<Option<SessionRuntime>>,
    status: RwLock<NetworkStatus>,
    signaling: SignalingConfig,
}

struct SessionRuntime {
    cancel: CancellationToken,
    tasks: Vec<JoinHandle<()>>,
}

impl NetworkManager {
    pub fn new() -> Self {
        let signaling = SignalingConfig::from_env();
        let mut status = NetworkStatus {
            signaling_server: signaling.broker_label(),
            ..Default::default()
        };
        status.logs.push("Приложение запущено.".into());

        Self {
            inner: Arc::new(Inner {
                control: Mutex::new(()),
                session: Mutex::new(None),
                status: RwLock::new(status),
                signaling,
            }),
        }
    }

    pub async fn get_status(&self) -> NetworkStatus {
        self.inner.status.read().await.clone()
    }

    pub async fn start_hosting(&self) -> Result<String> {
        let _guard = self.inner.control.lock().await;
        self.reset_session().await;

        match self.start_hosting_inner().await {
            Ok(room_code) => Ok(room_code),
            Err(error) => {
                self.mark_fatal(SessionMode::Host, None, &error).await;
                Err(error)
            }
        }
    }

    pub async fn connect_to_host(&self, room_code: String) -> Result<()> {
        let room_code = room_code.trim().to_uppercase();
        if room_code.is_empty() {
            return Err(anyhow!("room code must not be empty"));
        }

        let _guard = self.inner.control.lock().await;
        self.reset_session().await;

        match self.connect_to_host_inner(room_code.clone()).await {
            Ok(()) => Ok(()),
            Err(error) => {
                self.mark_fatal(SessionMode::Client, Some(room_code), &error)
                    .await;
                Err(error)
            }
        }
    }

    async fn start_hosting_inner(&self) -> Result<String> {
        let peer_id = Uuid::new_v4().to_string();
        let room_code = new_room_code();
        let peer_map = Arc::new(RwLock::new(HashMap::<SocketAddr, String>::new()));

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Host,
            state: ConnectionState::Starting,
            signaling_server: self.inner.signaling.broker_label(),
            note: Some("Узнаю внешний адрес и публикую room code.".into()),
            logs: vec![format!("Host mode стартовал. Room code: {room_code}")],
            ..Default::default()
        })
        .await;

        let (udp_socket, punch_socket, udp_bind_addr) = Self::bind_shared_udp_socket()?;
        let public_udp_addr =
            discover_public_addr(punch_socket.clone(), &self.inner.signaling).await?;
        let (server_config, server_cert) = build_server_config()?;

        let broker =
            BrokerConnection::connect(&self.inner.signaling, &format!("mc-host-{peer_id}"))
                .await
                .context("failed to connect to public signaling broker")?;
        subscribe_topic(&broker.client, &room_join_topic(&room_code)).await?;
        publish_broker_message(
            &broker.client,
            &room_offer_topic(&room_code),
            true,
            &BrokerMessage::HostOffer {
                room_code: room_code.clone(),
                peer_id: peer_id.clone(),
                peer_addr: public_udp_addr.to_string(),
                peer_cert: BASE64.encode(server_cert),
            },
        )
        .await?;

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
            room_code: Some(room_code.clone()),
            udp_bind_addr: Some(udp_bind_addr.to_string()),
            public_udp_addr: Some(public_udp_addr.to_string()),
            signaling_server: self.inner.signaling.broker_label(),
            note: Some("Комната опубликована. Отдайте room code другу.".into()),
            logs: vec![
                format!("Внешний адрес через STUN: {public_udp_addr}"),
                format!("MQTT broker: {}", self.inner.signaling.broker_label()),
            ],
            ..Default::default()
        })
        .await;

        let cancel = CancellationToken::new();
        let accept_task = self.spawn_host_accept_loop(endpoint, peer_map.clone(), cancel.clone());
        let signal_task = self.spawn_host_broker_loop(
            room_code.clone(),
            broker.client,
            broker.receiver,
            punch_socket,
            peer_map,
            cancel.clone(),
        );

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: vec![accept_task, signal_task],
        });

        Ok(room_code)
    }

    async fn connect_to_host_inner(&self, room_code: String) -> Result<()> {
        let peer_id = Uuid::new_v4().to_string();

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::Starting,
            room_code: Some(room_code.clone()),
            signaling_server: self.inner.signaling.broker_label(),
            note: Some("Узнаю внешний адрес и жду host offer.".into()),
            logs: vec![format!("Client mode стартовал. Комната: {room_code}")],
            ..Default::default()
        })
        .await;

        let (udp_socket, punch_socket, udp_bind_addr) = Self::bind_shared_udp_socket()?;
        let public_udp_addr =
            discover_public_addr(punch_socket.clone(), &self.inner.signaling).await?;

        let mut broker =
            BrokerConnection::connect(&self.inner.signaling, &format!("mc-client-{peer_id}"))
                .await
                .context("failed to connect to public signaling broker")?;
        subscribe_topic(&broker.client, &room_offer_topic(&room_code)).await?;
        subscribe_topic(&broker.client, &room_join_topic(&room_code)).await?;
        publish_broker_message(
            &broker.client,
            &room_join_topic(&room_code),
            false,
            &BrokerMessage::JoinRequest {
                room_code: room_code.clone(),
                peer_id: peer_id.clone(),
                peer_addr: public_udp_addr.to_string(),
            },
        )
        .await?;

        self.overwrite_status(NetworkStatus {
            mode: SessionMode::Client,
            state: ConnectionState::WaitingForPeer,
            room_code: Some(room_code.clone()),
            udp_bind_addr: Some(udp_bind_addr.to_string()),
            public_udp_addr: Some(public_udp_addr.to_string()),
            signaling_server: self.inner.signaling.broker_label(),
            note: Some("Join request отправлен. Жду host offer.".into()),
            logs: vec![
                format!("Внешний адрес через STUN: {public_udp_addr}"),
                format!("MQTT broker: {}", self.inner.signaling.broker_label()),
            ],
            ..Default::default()
        })
        .await;

        let (host_peer_id, host_addr, host_cert) = loop {
            match timeout(Duration::from_secs(12), broker.receiver.recv()).await {
                Ok(Some(BrokerMessage::HostOffer {
                    room_code: incoming_room,
                    peer_id,
                    peer_addr,
                    peer_cert,
                })) if incoming_room == room_code => {
                    break (peer_id, peer_addr.parse::<SocketAddr>()?, peer_cert);
                }
                Ok(Some(_)) => {}
                Ok(None) => return Err(anyhow!("signaling broker stream closed")),
                Err(_) => {
                    return Err(anyhow!(
                        "host offer not received. Room code invalid or host is offline"
                    ));
                }
            }
        };

        let mut endpoint = Endpoint::new(
            EndpointConfig::default(),
            None,
            udp_socket,
            Arc::new(quinn::TokioRuntime),
        )
        .context("failed to create client QUIC endpoint")?;
        endpoint.set_default_client_config(build_client_config(&BASE64.decode(host_cert)?)?);

        self.mutate_status(|status| {
            status.state = ConnectionState::Punching;
            status.peers = vec![PeerInfo {
                peer_id: host_peer_id.clone(),
                addr: host_addr.to_string(),
                connected: false,
                ping_ms: None,
            }];
            status.note = Some("Host найден. Пробиваю NAT.".into());
        })
        .await;

        let cancel = CancellationToken::new();
        let punch_handle = tokio::spawn({
            let socket = punch_socket.clone();
            let room = room_code.clone();
            let host_peer = host_peer_id.clone();
            let cancel = cancel.clone();
            async move {
                let _ = punch_remote(socket, host_addr, &room, &host_peer, cancel).await;
            }
        });

        let connection = self.connect_with_retries(&endpoint, host_addr).await?;
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
            status.peers = vec![PeerInfo {
                peer_id: host_peer_id.clone(),
                addr: host_addr.to_string(),
                connected: true,
                ping_ms: Some(connection.rtt().as_millis() as u64),
            }];
            status.note = Some("Подключено. Заходите в Minecraft на localhost.".into());
        })
        .await;
        self.push_log("Локальный proxy на 127.0.0.1:25565 поднят.".into())
            .await;

        let proxy_task =
            self.spawn_client_proxy_loop(local_listener, connection.clone(), cancel.clone());
        let ping_task =
            self.spawn_ping_loop(connection.clone(), host_peer_id.clone(), cancel.clone());
        let close_task =
            self.spawn_client_close_loop(connection.clone(), host_peer_id.clone(), cancel.clone());

        *self.inner.session.lock().await = Some(SessionRuntime {
            cancel,
            tasks: vec![proxy_task, ping_task, close_task],
        });

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
                            .push_log(format!("Peer {peer_id} подключился: {remote}"))
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
                                .set_nonfatal(format!(
                                    "входящее QUIC соединение отклонено: {error:#}"
                                ))
                                .await;
                        }
                    }
                }
            }
        })
    }

    fn spawn_host_broker_loop(
        &self,
        room_code: String,
        _client: AsyncClient,
        mut receiver: tokio::sync::mpsc::UnboundedReceiver<BrokerMessage>,
        punch_socket: Arc<UdpSocket>,
        peer_map: Arc<RwLock<HashMap<SocketAddr, String>>>,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                let message = tokio::select! {
                    _ = cancel.cancelled() => break,
                    message = receiver.recv() => message,
                };

                match message {
                    Some(BrokerMessage::JoinRequest {
                        room_code: incoming_room,
                        peer_id,
                        peer_addr,
                    }) if incoming_room == room_code => {
                        let addr: SocketAddr = match peer_addr.parse() {
                            Ok(addr) => addr,
                            Err(error) => {
                                manager
                                    .set_nonfatal(format!("некорректный адрес клиента: {error}"))
                                    .await;
                                continue;
                            }
                        };

                        peer_map.write().await.insert(addr, peer_id.clone());
                        manager
                            .upsert_peer(peer_id.clone(), addr, false, None)
                            .await;
                        manager
                            .push_log(format!("Join request от {peer_id}: {addr}"))
                            .await;

                        let punch_cancel = cancel.clone();
                        let socket = punch_socket.clone();
                        let room = room_code.clone();
                        tokio::spawn(async move {
                            let _ = punch_remote(socket, addr, &room, &peer_id, punch_cancel).await;
                        });
                    }
                    Some(_) => {}
                    None => break,
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
                                NetworkManager::handle_client_proxy_connection(conn, tcp_stream)
                                    .await
                            {
                                tracing::warn!("client proxy stream failed: {error:#}");
                            }
                        });
                    }
                    Err(error) => {
                        if !cancel.is_cancelled() {
                            manager
                                .set_nonfatal(format!("локальный proxy listener упал: {error:#}"))
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
                        &anyhow!("QUIC соединение закрыто: {error}"),
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
                        self.set_nonfatal(format!("peer stream closed with error: {error:#}"))
                            .await;
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
        host_addr: SocketAddr,
    ) -> Result<Connection> {
        let mut last_error = None;

        for attempt in 1..=6 {
            self.mutate_status(|status| {
                status.state = ConnectionState::Connecting;
                status.note = Some(format!("Handshake с хостом, попытка {attempt}/6."));
            })
            .await;

            let connect = endpoint
                .connect(host_addr, "localhost")
                .context("failed to start QUIC connect")?;

            match timeout(Duration::from_secs(4), connect).await {
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
            signaling_server: self.inner.signaling.broker_label(),
            ..Default::default()
        };
        status.logs.push("Сессия сброшена.".into());
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
            if status.logs.len() > 40 {
                status.logs.truncate(40);
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
                status.note = Some("Peer отключился, комната остаётся открытой.".into());
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
            signaling_server: self.inner.signaling.broker_label(),
            last_error: Some(formatted.clone()),
            note: Some("Сессия завершилась ошибкой.".into()),
            logs: vec![formatted],
            ..Default::default()
        })
        .await;
    }
}
