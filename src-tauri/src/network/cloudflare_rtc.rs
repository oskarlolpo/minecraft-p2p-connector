use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
};

use anyhow::{Context, Result};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;
use webrtc::{
    api::{interceptor_registry::register_default_interceptors, media_engine::MediaEngine, APIBuilder},
    data_channel::{
        data_channel_init::RTCDataChannelInit, data_channel_message::DataChannelMessage,
        RTCDataChannel,
    },
    ice_transport::ice_server::RTCIceServer,
    interceptor::registry::Registry,
    peer_connection::{
        configuration::RTCConfiguration,
        peer_connection_state::RTCPeerConnectionState,
        sdp::session_description::RTCSessionDescription,
        RTCPeerConnection,
    },
};

use crate::{
    diagnostics::DiagnosticsStore,
    models::{CloudflareAttempt, CloudflareRuntimeInfo, NetworkStatus, PeerInfo, TransportKind},
};

use super::{cloudflare::{CloudflareConfig, CloudflareIceServerResponse}, proxy};

const BOOTSTRAP_LABEL: &str = "mcp2p-bootstrap";
const STREAM_LABEL_PREFIX: &str = "mc-";
const DATA_CHUNK_SIZE: usize = 16 * 1024;

#[derive(Clone)]
pub struct CloudflareRtcManager {
    inner: Arc<Inner>,
}

struct Inner {
    config: CloudflareConfig,
    status: Arc<RwLock<NetworkStatus>>,
    diagnostics: DiagnosticsStore,
    sessions: Mutex<HashMap<String, Arc<RtcSession>>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RtcRole {
    Host,
    Client,
}

struct RtcSession {
    _session_id: String,
    _role: RtcRole,
    cancel: CancellationToken,
    peer_connection: Arc<RTCPeerConnection>,
    local_game_port: u16,
    listener_started: AtomicBool,
    next_stream_id: AtomicU64,
    listener_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct TunnelEvent {
    peer_addr: String,
    minecraft_addr: String,
    transport: String,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureEvent {
    reason: String,
}

impl CloudflareRtcManager {
    pub fn new(
        status: Arc<RwLock<NetworkStatus>>,
        diagnostics: DiagnosticsStore,
        config: CloudflareConfig,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                status,
                diagnostics,
                sessions: Mutex::new(HashMap::new()),
            }),
        }
    }

    pub async fn runtime_info(&self) -> CloudflareRuntimeInfo {
        match self.inner.config.probe_runtime().await {
            Ok(()) => CloudflareRuntimeInfo {
                ready: true,
                credential_endpoint: self.inner.config.credential_endpoint.clone(),
                turn_endpoint: self.inner.config.first_turn_endpoint(),
                note: "Cloudflare TURN credential backend reachable.".into(),
            },
            Err(error) => CloudflareRuntimeInfo {
                ready: false,
                credential_endpoint: self.inner.config.credential_endpoint.clone(),
                turn_endpoint: self.inner.config.first_turn_endpoint(),
                note: format!("{error:#}"),
            },
        }
    }

    pub async fn create_client_offer(
        &self,
        app: AppHandle,
        session_id: String,
        peer_addr: String,
    ) -> Result<String> {
        self.abort_session(&session_id).await;
        self.push_log("Cloudflare credentials requested for client session.".into())
            .await;
        let ice = self.fetch_ice_servers().await?;
        self.push_log("Cloudflare credentials ready.".into()).await;
        let peer_connection = build_peer_connection(&ice).await?;
        let session = Arc::new(RtcSession {
            _session_id: session_id.clone(),
            _role: RtcRole::Client,
            cancel: CancellationToken::new(),
            peer_connection: peer_connection.clone(),
            local_game_port: 25565,
            listener_started: AtomicBool::new(false),
            next_stream_id: AtomicU64::new(1),
            listener_task: Mutex::new(None),
        });
        self.inner
            .sessions
            .lock()
            .await
            .insert(session_id.clone(), session.clone());

        self.attach_common_handlers(app.clone(), session.clone(), peer_addr.clone())
            .await;
        self.attach_client_handlers(app.clone(), session.clone(), peer_addr.clone())
            .await?;

        let bootstrap = peer_connection
            .create_data_channel(
                BOOTSTRAP_LABEL,
                Some(RTCDataChannelInit {
                    ordered: Some(true),
                    ..Default::default()
                }),
            )
            .await
            .context("failed to create Cloudflare bootstrap data channel")?;
        self.attach_client_bootstrap(app.clone(), session, peer_addr, bootstrap)
            .await;

        let offer = peer_connection
            .create_offer(None)
            .await
            .context("failed to create Cloudflare offer")?;
        let mut gather_complete = peer_connection.gathering_complete_promise().await;
        peer_connection
            .set_local_description(offer)
            .await
            .context("failed to set local Cloudflare offer")?;
        let _ = gather_complete.recv().await;
        let local = peer_connection
            .local_description()
            .await
            .context("missing local Cloudflare offer")?;

        self.emit("cloudflare_credentials_ready", serde_json::json!({
            "sessionId": session_id,
            "iceServerCount": ice.ice_servers.len(),
        }), None)
        .await;
        self.mutate_status(|status| {
            status.transport_preference = Some("cloudflare".into());
            status.cloudflare_enabled = true;
            status.cloudflare_turn_ready = true;
            status.cloudflare_turn_endpoint = self.inner.config.first_turn_endpoint();
            status.note = Some(
                "Cloudflare WebRTC fallback поднимается. Сначала собираю ICE и TURN candidates."
                    .into(),
            );
        })
        .await;

        serde_json::to_string(&local).context("failed to serialize Cloudflare offer")
    }

    pub async fn accept_host_offer(
        &self,
        app: AppHandle,
        session_id: String,
        offer_json: String,
        local_game_port: u16,
        peer_addr: String,
    ) -> Result<String> {
        self.abort_session(&session_id).await;
        let ice = self.fetch_ice_servers().await?;
        let peer_connection = build_peer_connection(&ice).await?;
        let session = Arc::new(RtcSession {
            _session_id: session_id.clone(),
            _role: RtcRole::Host,
            cancel: CancellationToken::new(),
            peer_connection: peer_connection.clone(),
            local_game_port,
            listener_started: AtomicBool::new(false),
            next_stream_id: AtomicU64::new(1),
            listener_task: Mutex::new(None),
        });
        self.inner
            .sessions
            .lock()
            .await
            .insert(session_id.clone(), session.clone());

        self.attach_common_handlers(app.clone(), session.clone(), peer_addr.clone())
            .await;
        self.attach_host_handlers(app.clone(), session.clone(), peer_addr.clone())
            .await?;

        let offer = serde_json::from_str::<RTCSessionDescription>(&offer_json)
            .context("failed to decode Cloudflare offer")?;
        peer_connection
            .set_remote_description(offer)
            .await
            .context("failed to set remote Cloudflare offer")?;
        let answer = peer_connection
            .create_answer(None)
            .await
            .context("failed to create Cloudflare answer")?;
        let mut gather_complete = peer_connection.gathering_complete_promise().await;
        peer_connection
            .set_local_description(answer)
            .await
            .context("failed to set local Cloudflare answer")?;
        let _ = gather_complete.recv().await;
        let local = peer_connection
            .local_description()
            .await
            .context("missing local Cloudflare answer")?;

        self.mutate_status(|status| {
            status.cloudflare_enabled = true;
            status.cloudflare_turn_ready = true;
            status.cloudflare_turn_endpoint = self.inner.config.first_turn_endpoint();
            status.transport_preference = Some("cloudflare".into());
            status.note = Some(
                "Cloudflare answer создан. Жду установления WebRTC DataChannel от клиента."
                    .into(),
            );
        })
        .await;

        serde_json::to_string(&local).context("failed to serialize Cloudflare answer")
    }

    pub async fn finish_client_answer(
        &self,
        session_id: String,
        answer_json: String,
    ) -> Result<()> {
        let session = self
            .inner
            .sessions
            .lock()
            .await
            .get(&session_id)
            .cloned()
            .context("Cloudflare client session not found")?;
        let answer = serde_json::from_str::<RTCSessionDescription>(&answer_json)
            .context("failed to decode Cloudflare answer")?;
        session
            .peer_connection
            .set_remote_description(answer)
            .await
            .context("failed to apply Cloudflare answer")?;
        self.emit(
            "cloudflare_connecting",
            serde_json::json!({ "sessionId": session_id }),
            None,
        )
        .await;
        Ok(())
    }

    pub async fn abort_session(&self, session_id: &str) {
        if let Some(session) = self.inner.sessions.lock().await.remove(session_id) {
            session.cancel.cancel();
            if let Some(task) = session.listener_task.lock().await.take() {
                task.abort();
            }
            let _ = session.peer_connection.close().await;
        }
    }

    pub async fn abort_all(&self) {
        let keys = self
            .inner
            .sessions
            .lock()
            .await
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        for key in keys {
            self.abort_session(&key).await;
        }
    }

    async fn fetch_ice_servers(&self) -> Result<CloudflareIceServerResponse> {
        match self.inner.config.fetch_ice_servers().await {
            Ok(payload) => {
                self.inner
                    .diagnostics
                    .set_cloudflare_attempt(CloudflareAttempt {
                        transport: "cloudflare-webrtc".into(),
                        success: false,
                        detail: "Credentials fetched, ICE gathering pending".into(),
                        credential_status: "ok".into(),
                        selected_candidate_pair: Some(candidate_summary(&payload.ice_servers)),
                        endpoint: self.inner.config.first_turn_endpoint(),
                    })
                    .await;
                Ok(payload)
            }
            Err(error) => {
                self.inner
                    .diagnostics
                    .set_cloudflare_attempt(CloudflareAttempt {
                        transport: "cloudflare-webrtc".into(),
                        success: false,
                        detail: format!("{error:#}"),
                        credential_status: "failed".into(),
                        selected_candidate_pair: None,
                        endpoint: self.inner.config.first_turn_endpoint(),
                    })
                    .await;
                Err(error)
            }
        }
    }

    async fn attach_common_handlers(
        &self,
        app: AppHandle,
        session: Arc<RtcSession>,
        peer_addr: String,
    ) {
        let manager = self.clone();
        let peer_connection = session.peer_connection.clone();
        let session_for_cb = session.clone();
        peer_connection
            .on_peer_connection_state_change(Box::new(move |state: RTCPeerConnectionState| {
                let manager = manager.clone();
                let app = app.clone();
                let session = session_for_cb.clone();
                let peer_addr = peer_addr.clone();
                Box::pin(async move {
                    if matches!(
                        state,
                        RTCPeerConnectionState::Failed
                            | RTCPeerConnectionState::Closed
                            | RTCPeerConnectionState::Disconnected
                    ) && !session.cancel.is_cancelled()
                    {
                        manager
                            .inner
                            .diagnostics
                            .set_cloudflare_attempt(CloudflareAttempt {
                                transport: "cloudflare-webrtc".into(),
                                success: false,
                                detail: format!("Peer connection state: {state}"),
                                credential_status: "ok".into(),
                                selected_candidate_pair: None,
                                endpoint: manager.inner.config.first_turn_endpoint(),
                            })
                            .await;
                        manager
                            .emit(
                                "cloudflare_failed",
                                FailureEvent {
                                    reason: format!("Cloudflare peer connection failed: {state}"),
                                },
                                Some(&app),
                            )
                            .await;
                        manager
                            .set_nonfatal(format!(
                                "Cloudflare WebRTC failed for {peer_addr}: peer connection state {state}"
                            ))
                            .await;
                    }
                })
            }));
    }

    async fn attach_client_handlers(
        &self,
        app: AppHandle,
        session: Arc<RtcSession>,
        peer_addr: String,
    ) -> Result<()> {
        let manager = self.clone();
        let peer_connection = session.peer_connection.clone();
        let session_for_cb = session.clone();
        let peer_addr_for_cb = peer_addr.clone();
        let app_for_cb = app.clone();
        peer_connection
            .on_data_channel(Box::new(move |channel: Arc<RTCDataChannel>| {
                let manager = manager.clone();
                let session = session_for_cb.clone();
                let peer_addr = peer_addr_for_cb.clone();
                let app = app_for_cb.clone();
                Box::pin(async move {
                    let label = channel.label().to_owned();
                    if label == BOOTSTRAP_LABEL {
                        manager
                            .attach_client_bootstrap(app, session, peer_addr, channel)
                            .await;
                    }
                })
            }));

        app.emit("cloudflare_connecting", serde_json::json!({ "peerAddr": peer_addr }))
            .ok();
        Ok(())
    }

    async fn attach_host_handlers(
        &self,
        app: AppHandle,
        session: Arc<RtcSession>,
        peer_addr: String,
    ) -> Result<()> {
        let manager = self.clone();
        let peer_connection = session.peer_connection.clone();
        let session_for_cb = session.clone();
        let app_for_cb = app.clone();
        peer_connection
            .on_data_channel(Box::new(move |channel: Arc<RTCDataChannel>| {
                let manager = manager.clone();
                let session = session_for_cb.clone();
                let peer_addr = peer_addr.clone();
                let app = app_for_cb.clone();
                Box::pin(async move {
                    let label = channel.label().to_owned();
                    if label == BOOTSTRAP_LABEL {
                        let manager_open = manager.clone();
                        let session_open = session.clone();
                        let app_open = app.clone();
                        let peer_addr_open = peer_addr.clone();
                        let channel_for_open = channel.clone();
                        channel.on_open(Box::new(move || {
                            let manager = manager_open.clone();
                            let session = session_open.clone();
                            let app = app_open.clone();
                            let peer_addr = peer_addr_open.clone();
                            let _channel = channel_for_open.clone();
                            Box::pin(async move {
                                manager
                                    .migrate_status_to_cloudflare(&peer_addr, None)
                                    .await;
                                manager
                                    .push_log(format!(
                                        "Cloudflare bootstrap channel opened for {peer_addr}."
                                    ))
                                    .await;
                                manager
                                    .emit(
                                        "cloudflare_connected",
                                        TunnelEvent {
                                            peer_addr: peer_addr.clone(),
                                            minecraft_addr: format!(
                                                "127.0.0.1:{}",
                                                session.local_game_port
                                            ),
                                            transport: "cloudflare-webrtc".into(),
                                        },
                                        Some(&app),
                                    )
                                    .await;
                                session.listener_started.store(true, Ordering::Relaxed);
                            })
                        }));
                        return;
                    }

                    if label.starts_with(STREAM_LABEL_PREFIX) {
                        manager.attach_host_stream_channel(session, channel).await;
                    }
                })
            }));
        Ok(())
    }

    async fn attach_client_bootstrap(
        &self,
        app: AppHandle,
        session: Arc<RtcSession>,
        peer_addr: String,
        channel: Arc<RTCDataChannel>,
    ) {
        let manager = self.clone();
        let channel_for_open = channel.clone();
        channel.on_open(Box::new(move || {
            let manager = manager.clone();
            let session = session.clone();
            let peer_addr = peer_addr.clone();
            let app = app.clone();
            let _channel = channel_for_open.clone();
            Box::pin(async move {
                manager
                    .inner
                    .diagnostics
                    .set_cloudflare_attempt(CloudflareAttempt {
                        transport: "cloudflare-webrtc".into(),
                        success: true,
                        detail: "Cloudflare DataChannel opened".into(),
                        credential_status: "ok".into(),
                        selected_candidate_pair: Some("relay/turn".into()),
                        endpoint: manager.inner.config.first_turn_endpoint(),
                    })
                    .await;
                manager
                    .inner
                    .diagnostics
                    .set_selected_transport("cloudflare-webrtc")
                    .await;
                manager
                    .migrate_status_to_cloudflare(&peer_addr, Some(63))
                    .await;
                manager
                    .start_client_listener(app.clone(), session.clone(), peer_addr.clone())
                    .await;
                manager
                    .emit(
                        "cloudflare_connected",
                        TunnelEvent {
                            peer_addr: peer_addr.clone(),
                            minecraft_addr: proxy::MINECRAFT_LOCAL_ADDR.into(),
                            transport: "cloudflare-webrtc".into(),
                        },
                        Some(&app),
                    )
                    .await;
                manager
                    .emit(
                        "tunnel_established",
                        TunnelEvent {
                            peer_addr,
                            minecraft_addr: proxy::MINECRAFT_LOCAL_ADDR.into(),
                            transport: "cloudflare-webrtc".into(),
                        },
                        Some(&app),
                    )
                    .await;
            })
        }));
    }

    async fn attach_host_stream_channel(
        &self,
        session: Arc<RtcSession>,
        channel: Arc<RTCDataChannel>,
    ) {
        let local_port = session.local_game_port;
        let cancel = session.cancel.clone();
        let channel_for_open = channel.clone();
        channel.on_open(Box::new(move || {
            let channel = channel_for_open.clone();
            let cancel = cancel.clone();
            Box::pin(async move {
                let target = proxy::minecraft_local_addr(local_port);
                match TcpStream::connect(&target).await {
                    Ok(stream) => {
                        let (mut reader, writer) = stream.into_split();
                        let writer = Arc::new(Mutex::new(writer));
                        let write_side = writer.clone();
                        channel.on_message(Box::new(move |msg: DataChannelMessage| {
                            let writer = write_side.clone();
                            Box::pin(async move {
                                let mut guard = writer.lock().await;
                                let _ = guard.write_all(&msg.data).await;
                            })
                        }));

                        let channel_reader = channel.clone();
                        tokio::spawn(async move {
                            let mut buffer = vec![0u8; DATA_CHUNK_SIZE];
                            loop {
                                let read = tokio::select! {
                                    _ = cancel.cancelled() => break,
                                    read = reader.read(&mut buffer) => read,
                                };

                                match read {
                                    Ok(0) => break,
                                    Ok(size) => {
                                        if channel_reader
                                            .send(&buffer[..size].to_vec().into())
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                            let _ = channel_reader.close().await;
                        });
                    }
                    Err(error) => {
                        tracing::warn!("failed to connect host Minecraft target for Cloudflare channel: {error:#}");
                        let _ = channel.close().await;
                    }
                }
            })
        }));
    }

    async fn start_client_listener(&self, app: AppHandle, session: Arc<RtcSession>, peer_addr: String) {
        if session.listener_started.swap(true, Ordering::Relaxed) {
            return;
        }

        let bind = TcpListener::bind(proxy::MINECRAFT_LOCAL_ADDR).await;
        let listener = match bind {
            Ok(listener) => listener,
            Err(error) => {
                self.set_nonfatal(format!(
                    "Cloudflare local proxy bind failed on {}: {error:#}",
                    proxy::MINECRAFT_LOCAL_ADDR
                ))
                .await;
                self.emit(
                    "cloudflare_failed",
                    FailureEvent {
                        reason: format!("Local proxy bind failed: {error}"),
                    },
                    Some(&app),
                )
                .await;
                return;
            }
        };

        let manager = self.clone();
        let session_clone = session.clone();
        let task = tokio::spawn(async move {
            manager
                .push_log("Cloudflare local proxy is listening on 127.0.0.1:25565.".into())
                .await;
            loop {
                let incoming = tokio::select! {
                    _ = session_clone.cancel.cancelled() => break,
                    incoming = listener.accept() => incoming,
                };

                match incoming {
                    Ok((stream, _)) => {
                        let manager = manager.clone();
                        let session = session_clone.clone();
                        let peer_addr = peer_addr.clone();
                        tokio::spawn(async move {
                            if let Err(error) = manager
                                .open_stream_data_channel(session, stream, peer_addr)
                                .await
                            {
                                manager
                                    .set_nonfatal(format!(
                                        "Cloudflare TCP bridge failed: {error:#}"
                                    ))
                                    .await;
                            }
                        });
                    }
                    Err(error) => {
                        manager
                            .set_nonfatal(format!(
                                "Cloudflare local listener accept failed: {error:#}"
                            ))
                            .await;
                        break;
                    }
                }
            }
        });

        *session.listener_task.lock().await = Some(task);
    }

    async fn open_stream_data_channel(
        &self,
        session: Arc<RtcSession>,
        tcp_stream: TcpStream,
        peer_addr: String,
    ) -> Result<()> {
        let stream_id = session.next_stream_id.fetch_add(1, Ordering::Relaxed);
        let label = format!("{STREAM_LABEL_PREFIX}{stream_id}");
        let channel = session
            .peer_connection
            .create_data_channel(
                &label,
                Some(RTCDataChannelInit {
                    ordered: Some(true),
                    ..Default::default()
                }),
            )
            .await
            .with_context(|| format!("failed to create Cloudflare data channel {label}"))?;

        let (mut reader, writer) = tcp_stream.into_split();
        let writer = Arc::new(Mutex::new(writer));
        let write_side = writer.clone();
        channel.on_message(Box::new(move |msg: DataChannelMessage| {
            let writer = write_side.clone();
            Box::pin(async move {
                let mut guard = writer.lock().await;
                let _ = guard.write_all(&msg.data).await;
            })
        }));

        let channel_for_open = channel.clone();
        channel.on_open(Box::new(move || {
            let channel = channel_for_open.clone();
            let cancel = session.cancel.clone();
            Box::pin(async move {
                let mut buffer = vec![0u8; DATA_CHUNK_SIZE];
                loop {
                    let read = tokio::select! {
                        _ = cancel.cancelled() => break,
                        read = reader.read(&mut buffer) => read,
                    };

                    match read {
                        Ok(0) => break,
                        Ok(size) => {
                            if channel.send(&buffer[..size].to_vec().into()).await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let _ = channel.close().await;
            })
        }));

        self.push_log(format!(
            "Cloudflare stream channel {label} opened for {peer_addr}."
        ))
        .await;
        Ok(())
    }

    async fn migrate_status_to_cloudflare(&self, peer_addr: &str, ping_ms: Option<u64>) {
        self.mutate_status(|status| {
            status.state = crate::models::ConnectionState::Connected;
            status.transport_kind = TransportKind::CloudflareWebrtc;
            status.transport_path = Some("cloudflare-webrtc".into());
            status.transport_preference = Some("cloudflare".into());
            status.cloudflare_enabled = true;
            status.cloudflare_turn_ready = true;
            status.cloudflare_turn_endpoint = self.inner.config.first_turn_endpoint();
            status.note = Some(
                "Cloudflare TURN/WebRTC tunnel established. Подключайтесь в Minecraft к localhost:25565."
                    .into(),
            );
            status.peers = vec![PeerInfo {
                peer_id: "cloudflare-peer".into(),
                addr: peer_addr.into(),
                connected: true,
                ping_ms,
            }];
        })
        .await;
    }

    async fn push_log(&self, entry: String) {
        let mut status = self.inner.status.write().await;
        status.logs.insert(0, entry);
        if status.logs.len() > 96 {
            status.logs.truncate(96);
        }
    }

    async fn set_nonfatal(&self, message: String) {
        let mut status = self.inner.status.write().await;
        status.last_error = Some(message.clone());
        status.logs.insert(0, message);
        if status.logs.len() > 96 {
            status.logs.truncate(96);
        }
    }

    async fn mutate_status<F>(&self, update: F)
    where
        F: FnOnce(&mut NetworkStatus),
    {
        let mut status = self.inner.status.write().await;
        update(&mut status);
        status.peer_count = status.peers.iter().filter(|peer| peer.connected).count();
    }

    async fn emit<S: serde::Serialize + Clone>(
        &self,
        event: &str,
        payload: S,
        app: Option<&AppHandle>,
    ) {
        if let Some(app) = app {
            let _ = app.emit(event, payload);
        }
    }
}

async fn build_peer_connection(
    ice: &CloudflareIceServerResponse,
) -> Result<Arc<RTCPeerConnection>> {
    let mut media = MediaEngine::default();
    media.register_default_codecs()?;

    let mut registry = Registry::new();
    registry = register_default_interceptors(registry, &mut media)?;
    let api = APIBuilder::new()
        .with_media_engine(media)
        .with_interceptor_registry(registry)
        .build();

    let configuration = RTCConfiguration {
        ice_servers: ice
            .ice_servers
            .iter()
            .map(|server| RTCIceServer {
                urls: server.urls.clone(),
                username: server.username.clone().unwrap_or_default(),
                credential: server.credential.clone().unwrap_or_default(),
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };

    let peer_connection = Arc::new(
        api.new_peer_connection(configuration)
            .await
            .context("failed to create RTCPeerConnection")?,
    );
    Ok(peer_connection)
}

fn candidate_summary(servers: &[super::cloudflare::CloudflareIceServer]) -> String {
    let mut labels = Vec::new();
    for server in servers {
        for url in &server.urls {
            if url.contains("turn:") || url.contains("turns:") {
                labels.push("relay");
            } else if url.contains("stun:") {
                labels.push("srflx");
            }
        }
    }
    labels.sort_unstable();
    labels.dedup();
    if labels.is_empty() {
        "unknown".into()
    } else {
        labels.join("+")
    }
}
