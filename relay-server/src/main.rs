//! MCP2P Relay Server — Lightweight WebSocket relay for Minecraft P2P Connector.
//!
//! This server pairs a "host" and a "client" by `session_id`.
//! Once paired, all binary WebSocket frames are forwarded between them verbatim.
//!
//! Protocol (JSON text frames for handshake):
//!   Host  → Server:  { "type": "host_register", "session_id": "..." }
//!   Server → Host:   { "type": "registered",    "session_id": "..." }
//!   Client → Server: { "type": "client_join",   "session_id": "..." }
//!   Server → Client: { "type": "linked",        "session_id": "..." }
//!
//! After handshake, all binary frames are forwarded to the paired peer.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, RwLock},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};

// ── Configuration ───────────────────────────────────────────────────────

const LISTEN_ADDR: &str = "0.0.0.0:8443";
const SESSION_TTL: Duration = Duration::from_secs(3600); // 1 hour max session
const CHANNEL_SIZE: usize = 512;

// ── Types ───────────────────────────────────────────────────────────────

type Tx = mpsc::Sender<Message>;

#[derive(Debug)]
struct Session {
    host_tx: Tx,
    client_tx: Option<Tx>,
    created_at: Instant,
}

type Sessions = Arc<RwLock<HashMap<String, Session>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HandshakeMessage {
    HostRegister { session_id: String },
    ClientJoin { session_id: String },
    Registered { session_id: String },
    Linked { session_id: String },
    Error { message: String },
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mcp2p_relay=info".parse().unwrap()),
        )
        .init();

    let sessions: Sessions = Arc::new(RwLock::new(HashMap::new()));

    // Periodic cleanup of expired sessions
    let cleanup_sessions = sessions.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            let mut map = cleanup_sessions.write().await;
            let before = map.len();
            map.retain(|_, session| session.created_at.elapsed() < SESSION_TTL);
            let removed = before - map.len();
            if removed > 0 {
                tracing::info!("Cleaned up {removed} expired sessions ({} remaining)", map.len());
            }
        }
    });

    let listener = TcpListener::bind(LISTEN_ADDR).await.expect("failed to bind");
    tracing::info!("MCP2P Relay listening on {LISTEN_ADDR}");

    loop {
        let (stream, addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!("Accept error: {e}");
                continue;
            }
        };

        let sessions = sessions.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, addr, sessions).await {
                tracing::debug!("Connection {addr} ended: {e}");
            }
        });
    }
}

// ── Connection handler ──────────────────────────────────────────────────

async fn handle_connection(
    stream: TcpStream,
    addr: SocketAddr,
    sessions: Sessions,
) -> Result<(), Box<dyn std::error::Error>> {
    stream.set_nodelay(true)?;
    let ws = accept_async(stream).await?;
    let (mut ws_write, mut ws_read) = ws.split();

    tracing::debug!("New WebSocket from {addr}");

    // 1. Wait for handshake (first text message)
    let handshake_timeout = tokio::time::timeout(Duration::from_secs(10), ws_read.next()).await;
    let first_msg = match handshake_timeout {
        Ok(Some(Ok(Message::Text(text)))) => text,
        _ => return Err("handshake timeout or invalid first message".into()),
    };

    let msg: HandshakeMessage = serde_json::from_str(&first_msg)?;

    match msg {
        HandshakeMessage::HostRegister { session_id } => {
            tracing::info!("Host registered: session={session_id} from {addr}");

            let (tx, mut rx) = mpsc::channel::<Message>(CHANNEL_SIZE);

            // Register session
            sessions.write().await.insert(
                session_id.clone(),
                Session {
                    host_tx: tx,
                    client_tx: None,
                    created_at: Instant::now(),
                },
            );

            // Confirm registration
            let ack = serde_json::to_string(&HandshakeMessage::Registered {
                session_id: session_id.clone(),
            })?;
            ws_write.send(Message::Text(ack)).await?;

            // Relay loop: forward between host WS ↔ channel
            let mut ping_interval = tokio::time::interval(Duration::from_secs(20));
            ping_interval.tick().await; // discard immediate first tick
            loop {
                tokio::select! {
                    // Messages from host's WebSocket → forward to client
                    msg = ws_read.next() => {
                        let Some(msg) = msg else { break };
                        let msg = msg?;
                        match msg {
                            Message::Binary(data) => {
                                // Forward binary data to client
                                let sessions_read = sessions.read().await;
                                if let Some(session) = sessions_read.get(&session_id) {
                                    if let Some(client_tx) = &session.client_tx {
                                        let _ = client_tx.send(Message::Binary(data)).await;
                                    }
                                }
                            }
                            Message::Ping(data) => {
                                // Respond locally, do NOT forward
                                ws_write.send(Message::Pong(data)).await?;
                            }
                            Message::Pong(_) => {} // ignore
                            Message::Close(_) => break,
                            _ => {}
                        }
                    }
                    // Messages from channel (sent by client) → forward to host WS
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break };
                        ws_write.send(msg).await?;
                    }
                    // Server-side keepalive to prevent NAT timeout (50s issue)
                    _ = ping_interval.tick() => {
                        if ws_write.send(Message::Ping(vec![])).await.is_err() {
                            break;
                        }
                    }
                }
            }

            // Cleanup
            sessions.write().await.remove(&session_id);
            tracing::info!("Host disconnected: session={session_id}");
        }

        HandshakeMessage::ClientJoin { session_id } => {
            tracing::info!("Client joining: session={session_id} from {addr}");

            let (tx, mut rx) = mpsc::channel::<Message>(CHANNEL_SIZE);
            let host_tx;

            // Find session and link client
            {
                let mut sessions_write = sessions.write().await;
                let session = sessions_write.get_mut(&session_id);
                match session {
                    Some(session) => {
                        host_tx = session.host_tx.clone();
                        session.client_tx = Some(tx);
                    }
                    None => {
                        let err = serde_json::to_string(&HandshakeMessage::Error {
                            message: format!("session {session_id} not found"),
                        })?;
                        ws_write.send(Message::Text(err)).await?;
                        return Ok(());
                    }
                }
            }

            // Confirm link
            let ack = serde_json::to_string(&HandshakeMessage::Linked {
                session_id: session_id.clone(),
            })?;
            ws_write.send(Message::Text(ack)).await?;

            // Relay loop: forward between client WS ↔ channel
            let mut ping_interval = tokio::time::interval(Duration::from_secs(20));
            ping_interval.tick().await; // discard immediate first tick
            loop {
                tokio::select! {
                    // Messages from client's WebSocket → forward to host
                    msg = ws_read.next() => {
                        let Some(msg) = msg else { break };
                        let msg = msg?;
                        match msg {
                            Message::Binary(data) => {
                                let _ = host_tx.send(Message::Binary(data)).await;
                            }
                            Message::Ping(data) => {
                                // Respond locally, do NOT forward
                                ws_write.send(Message::Pong(data)).await?;
                            }
                            Message::Pong(_) => {} // ignore
                            Message::Close(_) => break,
                            _ => {}
                        }
                    }
                    // Messages from channel (sent by host) → forward to client WS
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break };
                        ws_write.send(msg).await?;
                    }
                    // Server-side keepalive to prevent NAT timeout
                    _ = ping_interval.tick() => {
                        if ws_write.send(Message::Ping(vec![])).await.is_err() {
                            break;
                        }
                    }
                }
            }

            // Cleanup client from session
            {
                let mut sessions_write = sessions.write().await;
                if let Some(session) = sessions_write.get_mut(&session_id) {
                    session.client_tx = None;
                }
            }
            tracing::info!("Client disconnected: session={session_id}");
        }

        _ => {
            let err = serde_json::to_string(&HandshakeMessage::Error {
                message: "expected host_register or client_join".into(),
            })?;
            ws_write.send(Message::Text(err)).await?;
        }
    }

    Ok(())
}
