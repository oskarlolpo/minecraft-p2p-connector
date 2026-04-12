use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, UdpSocket},
    sync::{mpsc, Mutex},
};
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    shared: Arc<Mutex<SharedState>>,
}

#[derive(Default)]
struct SharedState {
    rooms: HashMap<String, Room>,
    peers: HashMap<String, PeerSession>,
    tokens: HashMap<String, String>,
    /// WSS relay rooms: session_id → channel the host is waiting on.
    relay_rooms: HashMap<String, RelayHostWaiter>,
}

#[derive(Clone, Default)]
struct Room {
    host_id: String,
    clients: HashSet<String>,
    announced_clients: HashSet<String>,
}

struct PeerSession {
    peer_id: String,
    room_code: String,
    role: PeerRole,
    udp_token: String,
    server_cert: Option<String>,
    udp_addr: Option<SocketAddr>,
    sender: mpsc::UnboundedSender<Message>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PeerRole {
    Host,
    Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientSignal {
    CreateRoom {
        peer_id: String,
        udp_token: String,
        server_cert: String,
    },
    JoinRoom {
        peer_id: String,
        room_code: String,
        udp_token: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerSignal {
    RoomCreated {
        room_code: String,
    },
    PeerReady {
        room_code: String,
        peer_id: String,
        peer_addr: String,
        peer_cert: Option<String>,
        role: String,
    },
    PeerLeft {
        peer_id: String,
    },
    Error {
        message: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct UdpRegistration {
    token: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct UdpAck {
    ok: bool,
    token: String,
    observed_addr: String,
}

// ── WSS Relay types ─────────────────────────────────────────────────

/// A host waiting for a client to join its relay room.
struct RelayHostWaiter {
    /// Send half of the host's WebSocket, held until a client arrives.
    host_tx: mpsc::UnboundedSender<Message>,
    /// The host's receive stream, handed off to the bridge task.
    host_rx: Option<futures_util::stream::SplitStream<WebSocket>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RelayHandshake {
    HostRegister { session_id: String },
    ClientJoin { session_id: String },
    Registered { session_id: String },
    Linked { session_id: String },
    Error { message: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "minecraft_p2p_signaling_server=info".into()),
        )
        .init();

    let ws_addr = read_socket_addr("SIGNAL_WS_ADDR", "0.0.0.0:9001");
    let udp_addr = read_socket_addr("SIGNAL_UDP_ADDR", "0.0.0.0:9002");
    let state = AppState {
        shared: Arc::new(Mutex::new(SharedState::default())),
    };

    let udp_state = state.clone();
    tokio::spawn(async move {
        if let Err(error) = run_udp_server(udp_state, udp_addr).await {
            error!("UDP signaling server crashed: {error:#}");
        }
    });

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/relay", get(relay_ws_handler))
        .with_state(state.clone());

    let listener = TcpListener::bind(ws_addr)
        .await
        .with_context(|| format!("failed to bind websocket listener on {ws_addr}"))?;

    info!("signaling websocket listening on {ws_addr}");
    info!("signaling UDP listening on {udp_addr}");
    info!("WSS relay endpoint available at /relay");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(state, socket))
}

async fn handle_socket(state: AppState, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Message>();

    let send_task = tokio::spawn(async move {
        while let Some(message) = rx.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    });

    let init_message = loop {
        match receiver.next().await {
            Some(Ok(Message::Text(text))) => {
                match serde_json::from_str::<ClientSignal>(text.as_ref()) {
                    Ok(message) => break Some(message),
                    Err(error) => {
                        send_json(
                            &tx,
                            &ServerSignal::Error {
                                message: format!("invalid signaling message: {error}"),
                            },
                        );
                        break None;
                    }
                }
            }
            Some(Ok(Message::Binary(bytes))) => {
                match serde_json::from_slice::<ClientSignal>(bytes.as_ref()) {
                    Ok(message) => break Some(message),
                    Err(error) => {
                        send_json(
                            &tx,
                            &ServerSignal::Error {
                                message: format!("invalid signaling message: {error}"),
                            },
                        );
                        break None;
                    }
                }
            }
            Some(Ok(Message::Close(_))) | None => break None,
            Some(Ok(_)) => continue,
            Some(Err(error)) => {
                warn!("websocket receive error: {error}");
                break None;
            }
        }
    };

    let mut current_peer_id = None::<String>;

    if let Some(message) = init_message {
        match message {
            ClientSignal::CreateRoom {
                peer_id,
                udp_token,
                server_cert,
            } => {
                current_peer_id = Some(peer_id.clone());
                let room_code =
                    register_host(&state, &peer_id, udp_token, server_cert, tx.clone()).await;
                send_json(&tx, &ServerSignal::RoomCreated { room_code });
            }
            ClientSignal::JoinRoom {
                peer_id,
                udp_token,
                room_code,
            } => {
                current_peer_id = Some(peer_id.clone());
                match register_client(&state, &peer_id, &room_code, udp_token, tx.clone()).await {
                    Ok(()) => dispatch_ready_for_room(&state, &room_code).await,
                    Err(error) => send_json(&tx, &ServerSignal::Error { message: error }),
                }
            }
        }
    }

    while let Some(message) = receiver.next().await {
        match message {
            Ok(Message::Close(_)) => break,
            Ok(_) => {}
            Err(error) => {
                warn!("websocket stream error: {error}");
                break;
            }
        }
    }

    if let Some(peer_id) = current_peer_id {
        remove_peer(&state, &peer_id).await;
    }

    send_task.abort();
}

async fn run_udp_server(state: AppState, bind_addr: SocketAddr) -> Result<()> {
    let socket = UdpSocket::bind(bind_addr)
        .await
        .with_context(|| format!("failed to bind UDP signaling socket on {bind_addr}"))?;
    let mut buffer = [0u8; 2048];

    loop {
        let (size, peer_addr) = socket.recv_from(&mut buffer).await?;
        let registration: UdpRegistration = match serde_json::from_slice(&buffer[..size]) {
            Ok(value) => value,
            Err(error) => {
                warn!("invalid UDP registration payload from {peer_addr}: {error}");
                continue;
            }
        };

        let room_to_dispatch = {
            let mut shared = state.shared.lock().await;
            let Some(peer_id) = shared.tokens.get(&registration.token).cloned() else {
                let ack = serde_json::to_vec(&UdpAck {
                    ok: false,
                    token: registration.token.clone(),
                    observed_addr: peer_addr.to_string(),
                })?;
                socket.send_to(&ack, peer_addr).await?;
                continue;
            };

            let Some(peer) = shared.peers.get_mut(&peer_id) else {
                continue;
            };

            peer.udp_addr = Some(peer_addr);
            peer.room_code.clone()
        };

        let ack = serde_json::to_vec(&UdpAck {
            ok: true,
            token: registration.token,
            observed_addr: peer_addr.to_string(),
        })?;
        socket.send_to(&ack, peer_addr).await?;

        dispatch_ready_for_room(&state, &room_to_dispatch).await;
    }
}

async fn register_host(
    state: &AppState,
    peer_id: &str,
    udp_token: String,
    server_cert: String,
    sender: mpsc::UnboundedSender<Message>,
) -> String {
    let mut shared = state.shared.lock().await;
    let room_code = generate_room_code(&shared.rooms);

    shared.tokens.insert(udp_token.clone(), peer_id.into());
    shared.peers.insert(
        peer_id.into(),
        PeerSession {
            peer_id: peer_id.into(),
            room_code: room_code.clone(),
            role: PeerRole::Host,
            udp_token,
            server_cert: Some(server_cert),
            udp_addr: None,
            sender,
        },
    );
    shared.rooms.insert(
        room_code.clone(),
        Room {
            host_id: peer_id.into(),
            clients: HashSet::new(),
            announced_clients: HashSet::new(),
        },
    );

    room_code
}

async fn register_client(
    state: &AppState,
    peer_id: &str,
    room_code: &str,
    udp_token: String,
    sender: mpsc::UnboundedSender<Message>,
) -> std::result::Result<(), String> {
    let mut shared = state.shared.lock().await;
    if !shared.rooms.contains_key(room_code) {
        return Err("room code not found".into());
    }

    shared.tokens.insert(udp_token.clone(), peer_id.into());
    shared.peers.insert(
        peer_id.into(),
        PeerSession {
            peer_id: peer_id.into(),
            room_code: room_code.into(),
            role: PeerRole::Client,
            udp_token,
            server_cert: None,
            udp_addr: None,
            sender,
        },
    );
    if let Some(room) = shared.rooms.get_mut(room_code) {
        room.clients.insert(peer_id.into());
    }

    Ok(())
}

async fn dispatch_ready_for_room(state: &AppState, room_code: &str) {
    let notifications = {
        let mut shared = state.shared.lock().await;
        let Some(room_snapshot) = shared.rooms.get(room_code).cloned() else {
            return;
        };
        let Some(host) = shared.peers.get(&room_snapshot.host_id) else {
            return;
        };
        let Some(host_addr) = host.udp_addr else {
            return;
        };

        let host_peer_id = host.peer_id.clone();
        let host_cert = host.server_cert.clone();
        let host_sender = host.sender.clone();

        let mut ready_client_ids = Vec::new();
        let mut notifications = Vec::new();

        for client_id in &room_snapshot.clients {
            if room_snapshot.announced_clients.contains(client_id) {
                continue;
            }

            let Some(client) = shared.peers.get(client_id) else {
                continue;
            };
            let Some(client_addr) = client.udp_addr else {
                continue;
            };

            ready_client_ids.push(client_id.clone());
            notifications.push((
                host_sender.clone(),
                ServerSignal::PeerReady {
                    room_code: room_code.into(),
                    peer_id: client.peer_id.clone(),
                    peer_addr: client_addr.to_string(),
                    peer_cert: None,
                    role: "client".into(),
                },
            ));
            notifications.push((
                client.sender.clone(),
                ServerSignal::PeerReady {
                    room_code: room_code.into(),
                    peer_id: host_peer_id.clone(),
                    peer_addr: host_addr.to_string(),
                    peer_cert: host_cert.clone(),
                    role: "host".into(),
                },
            ));
        }

        if let Some(room) = shared.rooms.get_mut(room_code) {
            for client_id in ready_client_ids {
                room.announced_clients.insert(client_id);
            }
        }

        notifications
    };

    for (sender, message) in notifications {
        send_json(&sender, &message);
    }
}

async fn remove_peer(state: &AppState, peer_id: &str) {
    let notifications = {
        let mut shared = state.shared.lock().await;
        let Some(peer) = shared.peers.remove(peer_id) else {
            return;
        };

        shared.tokens.remove(&peer.udp_token);

        match peer.role {
            PeerRole::Host => {
                let Some(room) = shared.rooms.remove(&peer.room_code) else {
                    return;
                };

                let mut notifications = Vec::new();
                for client_id in room.clients {
                    if let Some(client) = shared.peers.remove(&client_id) {
                        shared.tokens.remove(&client.udp_token);
                        notifications.push((
                            client.sender.clone(),
                            ServerSignal::PeerLeft {
                                peer_id: peer.peer_id.clone(),
                            },
                        ));
                    }
                }

                notifications
            }
            PeerRole::Client => {
                let mut notifications = Vec::new();
                let host_id = if let Some(room) = shared.rooms.get_mut(&peer.room_code) {
                    room.clients.remove(peer_id);
                    room.announced_clients.remove(peer_id);
                    Some(room.host_id.clone())
                } else {
                    None
                };

                if let Some(host_id) = host_id {
                    if let Some(host) = shared.peers.get(&host_id) {
                        notifications.push((
                            host.sender.clone(),
                            ServerSignal::PeerLeft {
                                peer_id: peer.peer_id.clone(),
                            },
                        ));
                    }
                }
                notifications
            }
        }
    };

    for (sender, message) in notifications {
        send_json(&sender, &message);
    }
}

fn send_json(sender: &mpsc::UnboundedSender<Message>, message: &ServerSignal) {
    match serde_json::to_string(message) {
        Ok(payload) => {
            let _ = sender.send(Message::Text(payload.into()));
        }
        Err(error) => warn!("failed to serialize signaling message: {error}"),
    }
}

fn generate_room_code(rooms: &HashMap<String, Room>) -> String {
    loop {
        let raw = Uuid::new_v4().simple().to_string();
        let code = raw[..6].to_uppercase();
        if !rooms.contains_key(&code) {
            return code;
        }
    }
}

fn read_socket_addr(key: &str, fallback: &str) -> SocketAddr {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| {
            fallback
                .parse()
                .expect("fallback socket addr must be valid")
        })
}

// ── WSS Relay Logic ──────────────────────────────────────────────────

async fn relay_ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_relay_socket(state, socket))
}

async fn handle_relay_socket(state: AppState, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    // 1. Wait for handshake
    let init_message = match receiver.next().await {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<RelayHandshake>(&text) {
            Ok(msg) => msg,
            Err(e) => {
                let _ = sender.send(Message::Text(serde_json::to_string(&RelayHandshake::Error { message: e.to_string() }).unwrap())).await;
                return;
            }
        },
        _ => return, // Invalid or closed
    };

    match init_message {
        RelayHandshake::HostRegister { session_id } => {
            let (tx, mut rx) = mpsc::unbounded_channel();
            
            // Register host in shared state
            {
                let mut shared = state.shared.lock().await;
                shared.relay_rooms.insert(session_id.clone(), RelayHostWaiter {
                    host_tx: tx,
                    host_rx: Some(receiver), // Transfer ownership of the read half
                });
            }

            // Acknowledge registration
            if sender.send(Message::Text(serde_json::to_string(&RelayHandshake::Registered { session_id: session_id.clone() }).unwrap())).await.is_err() {
                // Host disconnected immediately
                state.shared.lock().await.relay_rooms.remove(&session_id);
                return;
            }

            // Start draining messages from rx to the host's sender half
            while let Some(msg) = rx.recv().await {
                if sender.send(msg).await.is_err() {
                    break;
                }
            }

            // Cleanup when host disconnects
            state.shared.lock().await.relay_rooms.remove(&session_id);
        }
        RelayHandshake::ClientJoin { session_id } => {
            // Find the host
            let host_waiter = {
                let mut shared = state.shared.lock().await;
                shared.relay_rooms.remove(&session_id)
            };

            let Some(mut host_waiter) = host_waiter else {
                let _ = sender.send(Message::Text(serde_json::to_string(&RelayHandshake::Error { message: "Room not found or host left".into() }).unwrap())).await;
                return;
            };

            // Acknowledge to client
            if sender.send(Message::Text(serde_json::to_string(&RelayHandshake::Linked { session_id: session_id.clone() }).unwrap())).await.is_err() {
                return; // Client disconnected
            }

            // We have both halves. We need to bridge them.
            // Client: `sender`, `receiver`
            // Host: `host_waiter.host_tx`, `host_waiter.host_rx` (Option)
            
            let Some(mut host_rx) = host_waiter.host_rx.take() else { return };
            let host_tx = host_waiter.host_tx;

            let bridge_task_1 = tokio::spawn(async move {
                while let Some(Ok(msg)) = receiver.next().await {
                    if host_tx.send(msg).is_err() { break; }
                }
            });

            let bridge_task_2 = tokio::spawn(async move {
                while let Some(Ok(msg)) = host_rx.next().await {
                    if sender.send(msg).await.is_err() { break; }
                }
            });

            // Wait for either to close
            tokio::select! {
                _ = bridge_task_1 => {}
                _ = bridge_task_2 => {}
            }
        }
        _ => {
            // Unexpected initial message
            let _ = sender.send(Message::Text(serde_json::to_string(&RelayHandshake::Error { message: "Expected HostRegister or ClientJoin".into() }).unwrap())).await;
        }
    }
}

