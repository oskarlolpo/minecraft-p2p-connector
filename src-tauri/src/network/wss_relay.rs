//! WSS (WebSocket Secure) relay transport.
//!
//! This module provides a reliable fallback for networks where UDP/QUIC is
//! blocked (symmetric NAT, DPI, public Wi-Fi). It tunnels multiplexed
//! Minecraft TCP streams over a single WebSocket connection to a relay server.
//!
//! Protocol (binary frames over WebSocket):
//!   [1 byte kind] [8 bytes stream_id BE] [N bytes payload]
//!
//! Frame kinds match `relay.rs` for consistency:
//!   1 = OPEN   — request to open a new virtual stream
//!   2 = DATA   — raw TCP payload for a stream
//!   3 = CLOSE  — graceful stream teardown
//!   4 = HELLO  — initial handshake from client side
//!   5 = READY  — host acknowledges client is linked
//!   6 = ERROR  — stream-level error with message
//!   7 = PING   — keepalive (WSS-level, not TCP-level)

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Mutex},
    task::JoinHandle,
    time::timeout,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;

use super::proxy;

// ── Frame constants (shared with relay.rs) ──────────────────────────────

const FRAME_OPEN: u8 = 1;
const FRAME_DATA: u8 = 2;
const FRAME_CLOSE: u8 = 3;
const FRAME_HELLO: u8 = 4;
const FRAME_READY: u8 = 5;
const FRAME_ERROR: u8 = 6;
const FRAME_PING: u8 = 7;
const FRAME_DATA_ZSTD: u8 = 8; // Zstd-compressed DATA frame

// Only compress DATA payloads above this threshold (bytes).
// Small frames have negligible savings but add CPU overhead.
const ZSTD_MIN_PAYLOAD: usize = 64;
const ZSTD_LEVEL: i32 = 1; // fastest compression

// ── Tunables ────────────────────────────────────────────────────────────

const TCP_CHUNK_SIZE: usize = 16 * 1024;
const WS_WRITE_QUEUE: usize = 256;
const TCP_WRITE_QUEUE: usize = 64;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READY_TIMEOUT: Duration = Duration::from_secs(12);
const RECONNECT_DELAY: Duration = Duration::from_millis(2_000);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(25);

const DEFAULT_WSS_RELAY_URL: &str = "ws://2.26.87.126:8443/ws";

// ── Public types ────────────────────────────────────────────────────────

type StreamMap = Arc<Mutex<HashMap<u64, mpsc::Sender<Vec<u8>>>>>;

/// Configuration for connecting to a WSS relay.
#[derive(Clone, Debug)]
pub struct WssRelayConfig {
    /// Full WSS URL of the relay endpoint (e.g. `wss://relay.example.com/ws`).
    pub relay_url: String,
    /// Session/room identifier shared between host and client.
    pub session_id: String,
}

/// Opaque handle to a running WSS relay session.
pub struct WssRelayRuntime {
    join_handle: JoinHandle<Result<()>>,
}

/// JSON envelope sent during the WebSocket handshake phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum HandshakeMessage {
    /// Host registers a new relay room.
    HostRegister { session_id: String },
    /// Client joins an existing relay room.
    ClientJoin { session_id: String },
    /// Server confirms the room is ready.
    Registered { session_id: String },
    /// Server confirms the client is linked to a host.
    Linked { session_id: String },
    /// Server-side error.
    Error { message: String },
}

// ── Config helpers ──────────────────────────────────────────────────────

impl WssRelayConfig {
    /// Build configuration from environment or hardcoded defaults.
    pub fn from_env(session_id: String) -> Self {
        Self {
            relay_url: std::env::var("MC_WSS_RELAY_URL")
                .unwrap_or_else(|_| DEFAULT_WSS_RELAY_URL.into()),
            session_id,
        }
    }
}

impl WssRelayRuntime {
    /// Block until the relay session ends (or errors).
    pub async fn wait(self) -> Result<()> {
        self.join_handle
            .await
            .context("WSS relay task panicked")?
    }

    /// Abort the relay task.
    pub fn abort(&self) {
        self.join_handle.abort();
    }
}

// ═══════════════════════════════════════════════════════════════════════
//  HOST SIDE
// ═══════════════════════════════════════════════════════════════════════

/// Start a WSS relay session in **host** mode.
///
/// The host connects to the relay via WSS, registers its `session_id`,
/// and then forwards incoming virtual streams to the local Minecraft
/// server on `local_game_port`.
pub async fn start_host_runtime(
    config: WssRelayConfig,
    local_game_port: u16,
    cancel: CancellationToken,
) -> Result<WssRelayRuntime> {
    let (ready_tx, ready_rx) = oneshot::channel::<Result<()>>();
    let join_handle = tokio::spawn(host_loop(config, local_game_port, cancel, Some(ready_tx)));
    timeout(READY_TIMEOUT, ready_rx)
        .await
        .context("WSS relay host registration timed out")?
        .context("WSS relay host ready channel dropped")??;
    Ok(WssRelayRuntime { join_handle })
}

async fn host_loop(
    config: WssRelayConfig,
    local_game_port: u16,
    cancel: CancellationToken,
    mut ready_signal: Option<oneshot::Sender<Result<()>>>,
) -> Result<()> {
    loop {
        if cancel.is_cancelled() {
            return Ok(());
        }

        let result = host_session(&config, local_game_port, cancel.clone(), &mut ready_signal).await;

        if cancel.is_cancelled() {
            return Ok(());
        }

        match result {
            Ok(()) => {
                tracing::info!("WSS relay host session ended cleanly, reconnecting...");
            }
            Err(error) => {
                if let Some(tx) = ready_signal.take() {
                    let _ = tx.send(Err(anyhow!("{error:#}")));
                }
                tracing::warn!("WSS relay host session error: {error:#}, reconnecting...");
            }
        }

        tokio::time::sleep(RECONNECT_DELAY).await;
    }
}

async fn host_session(
    config: &WssRelayConfig,
    local_game_port: u16,
    cancel: CancellationToken,
    ready_signal: &mut Option<oneshot::Sender<Result<()>>>,
) -> Result<()> {
    // 1. Connect to relay
    let (ws_stream, _) = timeout(CONNECT_TIMEOUT, connect_async(&config.relay_url))
        .await
        .context("WSS relay connect timed out")?
        .context("WSS relay connect failed")?;

    let (mut ws_write, mut ws_read) = ws_stream.split();

    // 2. Send registration handshake
    let register = serde_json::to_string(&HandshakeMessage::HostRegister {
        session_id: config.session_id.clone(),
    })?;
    ws_write.send(Message::Text(register)).await?;

    // 3. Wait for confirmation
    let ack = timeout(CONNECT_TIMEOUT, ws_read.next())
        .await
        .context("WSS relay registration timed out")?
        .ok_or_else(|| anyhow!("WSS relay closed during registration"))?
        .context("WSS relay read error during registration")?;

    match ack {
        Message::Text(text) => {
            let msg: HandshakeMessage = serde_json::from_str(&text)?;
            match msg {
                HandshakeMessage::Registered { session_id } => {
                    tracing::info!("WSS relay host registered: session={session_id}");
                    if let Some(tx) = ready_signal.take() {
                        let _ = tx.send(Ok(()));
                    }
                }
                HandshakeMessage::Error { message } => {
                    return Err(anyhow!("WSS relay error: {message}"));
                }
                other => {
                    return Err(anyhow!("unexpected WSS handshake: {other:?}"));
                }
            }
        }
        _ => return Err(anyhow!("unexpected WSS message type during handshake")),
    }

    // 4. Set up multiplexed stream handling
    let streams: StreamMap = Arc::new(Mutex::new(HashMap::new()));
    let (ws_tx, mut ws_rx) = mpsc::channel::<Message>(WS_WRITE_QUEUE);

    // Writer task — drains ws_tx into the WebSocket
    let write_cancel = cancel.clone();
    let write_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = write_cancel.cancelled() => break,
                msg = ws_rx.recv() => {
                    let Some(msg) = msg else { break };
                    if ws_write.send(msg).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Keepalive task
    let ping_tx = ws_tx.clone();
    let ping_cancel = cancel.clone();
    let ping_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = ping_cancel.cancelled() => break,
                _ = tokio::time::sleep(KEEPALIVE_INTERVAL) => {
                    let frame = encode_frame(FRAME_PING, 0, &[]);
                    if ping_tx.send(Message::Binary(frame)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Reader loop — process incoming binary frames
    loop {
        let msg = tokio::select! {
            _ = cancel.cancelled() => break,
            msg = ws_read.next() => msg,
        };

        let Some(msg) = msg else { break };
        let msg = msg.context("WSS relay read error")?;

        match msg {
            Message::Binary(data) => {
                handle_host_frame(
                    &data,
                    &ws_tx,
                    &streams,
                    local_game_port,
                    cancel.clone(),
                )
                .await?;
            }
            Message::Ping(data) => {
                let _ = ws_tx.send(Message::Pong(data)).await;
            }
            Message::Close(_) => break,
            _ => {} // ignore text frames in data phase
        }
    }

    write_task.abort();
    ping_task.abort();
    Ok(())
}

async fn handle_host_frame(
    raw: &[u8],
    ws_tx: &mpsc::Sender<Message>,
    streams: &StreamMap,
    local_game_port: u16,
    cancel: CancellationToken,
) -> Result<()> {
    let frame = decode_frame(raw)?;

    match frame.kind {
        FRAME_HELLO => {
            // Client says hello → respond with READY
            let ack = encode_frame(FRAME_READY, 0, &[]);
            let _ = ws_tx.send(Message::Binary(ack)).await;
        }
        FRAME_OPEN => {
            if streams.lock().await.contains_key(&frame.stream_id) {
                return Ok(());
            }

            let target_addr = proxy::minecraft_local_addr(local_game_port);
            match TcpStream::connect(&target_addr).await {
                Ok(tcp_stream) => {
                    let (reader, writer) = tcp_stream.into_split();
                    let (tx, rx) = mpsc::channel::<Vec<u8>>(TCP_WRITE_QUEUE);
                    streams.lock().await.insert(frame.stream_id, tx);

                    // TCP writer — drains channel into TCP
                    let w_streams = streams.clone();
                    let w_cancel = cancel.clone();
                    let w_stream_id = frame.stream_id;
                    tokio::spawn(async move {
                        pump_channel_to_writer(writer, rx, w_streams, w_stream_id, w_cancel).await;
                    });

                    // TCP reader — pumps TCP into WebSocket
                    let r_ws_tx = ws_tx.clone();
                    let r_streams = streams.clone();
                    let r_cancel = cancel.clone();
                    let r_stream_id = frame.stream_id;
                    tokio::spawn(async move {
                        pump_reader_to_ws(reader, r_ws_tx, r_streams, r_stream_id, r_cancel).await;
                    });
                }
                Err(error) => {
                    let err_frame = encode_frame(
                        FRAME_ERROR,
                        frame.stream_id,
                        error.to_string().as_bytes(),
                    );
                    let _ = ws_tx.send(Message::Binary(err_frame)).await;
                    let close_frame = encode_frame(FRAME_CLOSE, frame.stream_id, &[]);
                    let _ = ws_tx.send(Message::Binary(close_frame)).await;
                }
            }
        }
        FRAME_DATA => {
            if let Some(tx) = streams.lock().await.get(&frame.stream_id).cloned() {
                let _ = tx.send(frame.payload.to_vec()).await;
            }
        }
        FRAME_CLOSE | FRAME_ERROR => {
            streams.lock().await.remove(&frame.stream_id);
        }
        FRAME_PING | FRAME_READY => {} // no-op
        _ => {
            tracing::warn!("WSS relay host: unknown frame kind {}", frame.kind);
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
//  CLIENT SIDE
// ═══════════════════════════════════════════════════════════════════════

/// Start a WSS relay session in **client** mode.
///
/// The client connects to the relay, joins the host's room, then opens a
/// local TCP listener on `127.0.0.1:25565`. Each incoming Minecraft
/// connection is multiplexed over the WebSocket to the host.
pub async fn start_client_runtime(
    config: WssRelayConfig,
    cancel: CancellationToken,
) -> Result<(WssRelayRuntime, u16)> {
    let temp_listener = TcpListener::bind("127.0.0.1:0").await?;
    let local_port = temp_listener.local_addr()?.port();
    drop(temp_listener);
    // Verify connectivity and handshake before returning
    let (ready_tx, ready_rx) = oneshot::channel::<Result<()>>();

    let join_handle = tokio::spawn(client_loop(config, local_port, cancel, Some(ready_tx)));

    // Wait for the relay to become ready (or fail fast)
    timeout(READY_TIMEOUT, ready_rx)
        .await
        .context("WSS relay client ready timed out")?
        .context("WSS relay client ready channel dropped")??;

    Ok((WssRelayRuntime { join_handle }, local_port))
}

async fn client_loop(
    config: WssRelayConfig,
    local_port: u16,
    cancel: CancellationToken,
    mut ready_signal: Option<oneshot::Sender<Result<()>>>,
) -> Result<()> {
    loop {
        if cancel.is_cancelled() {
            return Ok(());
        }

        let result = client_session(&config, local_port, cancel.clone(), &mut ready_signal).await;

        if cancel.is_cancelled() {
            return Ok(());
        }

        match result {
            Ok(()) => {
                tracing::info!("WSS relay client session ended cleanly, reconnecting...");
            }
            Err(error) => {
                if let Some(tx) = ready_signal.take() {
                    let _ = tx.send(Err(anyhow!("{error:#}")));
                }
                tracing::warn!("WSS relay client session error: {error:#}, reconnecting...");
            }
        }

        tokio::time::sleep(RECONNECT_DELAY).await;
    }
}

async fn client_session(
    config: &WssRelayConfig,
    local_port: u16,
    cancel: CancellationToken,
    ready_signal: &mut Option<oneshot::Sender<Result<()>>>,
) -> Result<()> {
    // 1. Connect to relay
    let (ws_stream, _) = timeout(CONNECT_TIMEOUT, connect_async(&config.relay_url))
        .await
        .context("WSS relay connect timed out")?
        .context("WSS relay connect failed")?;

    let (mut ws_write, mut ws_read) = ws_stream.split();

    // 2. Send join handshake
    let join_msg = serde_json::to_string(&HandshakeMessage::ClientJoin {
        session_id: config.session_id.clone(),
    })?;
    ws_write.send(Message::Text(join_msg)).await?;

    // 3. Wait for link confirmation
    let ack = timeout(CONNECT_TIMEOUT, ws_read.next())
        .await
        .context("WSS relay join timed out (check signaling server /relay endpoint)")?
        .ok_or_else(|| anyhow!("WSS relay closed during join (signaling server rejected connection)"))?
        .context("WSS relay read error during join")?;

    match ack {
        Message::Text(text) => {
            let msg: HandshakeMessage = serde_json::from_str(&text)?;
            match msg {
                HandshakeMessage::Linked { session_id } => {
                    tracing::info!("WSS relay client linked: session={session_id}");
                }
                HandshakeMessage::Error { message } => {
                    return Err(anyhow!("WSS relay error: {message}"));
                }
                other => {
                    return Err(anyhow!("unexpected WSS handshake: {other:?}"));
                }
            }
        }
        _ => return Err(anyhow!("unexpected WSS message type during join")),
    }

    // 4. Send HELLO and wait for READY
    let hello = encode_frame(FRAME_HELLO, 0, &[]);
    ws_write.send(Message::Binary(hello)).await?;

    // 5. Set up multiplexed streams
    let streams: StreamMap = Arc::new(Mutex::new(HashMap::new()));
    let stream_counter = Arc::new(AtomicU64::new(1));
    let (ws_tx, mut ws_rx) = mpsc::channel::<Message>(WS_WRITE_QUEUE);

    // Writer task
    let write_cancel = cancel.clone();
    let write_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = write_cancel.cancelled() => break,
                msg = ws_rx.recv() => {
                    let Some(msg) = msg else { break };
                    if ws_write.send(msg).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Keepalive task
    let ping_tx = ws_tx.clone();
    let ping_cancel = cancel.clone();
    let ping_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = ping_cancel.cancelled() => break,
                _ = tokio::time::sleep(KEEPALIVE_INTERVAL) => {
                    let frame = encode_frame(FRAME_PING, 0, &[]);
                    if ping_tx.send(Message::Binary(frame)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // TCP accept loop — listen for local Minecraft connections
    let listener_addr = format!("127.0.0.1:{local_port}");
    let listener = TcpListener::bind(&listener_addr)
        .await
        .with_context(|| {
            format!(
                "WSS relay: не удалось занять порт {}",
                listener_addr
            )
        })?;

    let accept_tx = ws_tx.clone();
    let accept_streams = streams.clone();
    let accept_counter = stream_counter.clone();
    let accept_cancel = cancel.clone();
    let accept_task = tokio::spawn(async move {
        loop {
            let incoming = tokio::select! {
                _ = accept_cancel.cancelled() => break,
                incoming = listener.accept() => incoming,
            };

            let (tcp_stream, peer) = match incoming {
                Ok(v) => v,
                Err(error) => {
                    tracing::warn!("WSS relay accept error: {error}");
                    continue;
                }
            };

            let stream_id = accept_counter.fetch_add(1, Ordering::Relaxed);
            tracing::debug!("WSS relay client: new Minecraft connection from {peer}, stream_id={stream_id}");

            // Send OPEN frame
            let open = encode_frame(FRAME_OPEN, stream_id, &[]);
            if accept_tx.send(Message::Binary(open)).await.is_err() {
                break;
            }

            let (reader, writer) = tcp_stream.into_split();
            let (tx, rx) = mpsc::channel::<Vec<u8>>(TCP_WRITE_QUEUE);
            accept_streams.lock().await.insert(stream_id, tx);

            // TCP writer
            let w_streams = accept_streams.clone();
            let w_cancel = accept_cancel.clone();
            tokio::spawn(async move {
                pump_channel_to_writer(writer, rx, w_streams, stream_id, w_cancel).await;
            });

            // TCP reader → WS
            let r_tx = accept_tx.clone();
            let r_streams = accept_streams.clone();
            let r_cancel = accept_cancel.clone();
            tokio::spawn(async move {
                pump_reader_to_ws(reader, r_tx, r_streams, stream_id, r_cancel).await;
            });
        }
    });

    // Signal readiness
    let mut got_ready = false;

    // Reader loop — process incoming binary frames from host
    loop {
        let msg = tokio::select! {
            _ = cancel.cancelled() => break,
            msg = ws_read.next() => msg,
        };

        let Some(msg) = msg else { break };
        let msg = msg.context("WSS relay client read error")?;

        match msg {
            Message::Binary(data) => {
                let frame = decode_frame(&data)?;
                match frame.kind {
                    FRAME_READY => {
                        if !got_ready {
                            got_ready = true;
                            if let Some(tx) = ready_signal.take() {
                                let _ = tx.send(Ok(()));
                            }
                        }
                    }
                    FRAME_DATA => {
                        if let Some(tx) = streams.lock().await.get(&frame.stream_id).cloned() {
                            let _ = tx.send(frame.payload.to_vec()).await;
                        }
                    }
                    FRAME_CLOSE | FRAME_ERROR => {
                        streams.lock().await.remove(&frame.stream_id);
                    }
                    FRAME_PING => {} // no-op for keepalive
                    _ => {
                        tracing::debug!("WSS relay client: ignoring frame kind {}", frame.kind);
                    }
                }
            }
            Message::Ping(data) => {
                let _ = ws_tx.send(Message::Pong(data)).await;
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    write_task.abort();
    ping_task.abort();
    accept_task.abort();
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════
//  SHARED I/O PUMPS
// ═══════════════════════════════════════════════════════════════════════

/// Read from a TCP half and send DATA frames over the WebSocket.
async fn pump_reader_to_ws<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    ws_tx: mpsc::Sender<Message>,
    streams: StreamMap,
    stream_id: u64,
    cancel: CancellationToken,
) {
    let mut buffer = vec![0u8; TCP_CHUNK_SIZE];

    loop {
        let n = tokio::select! {
            _ = cancel.cancelled() => break,
            n = reader.read(&mut buffer) => n,
        };

        match n {
            Ok(0) => break,
            Ok(size) => {
                let frame = encode_data_frame_maybe_compressed(stream_id, &buffer[..size]);
                if ws_tx.send(Message::Binary(frame)).await.is_err() {
                    break;
                }
            }
            Err(_) => break,
        }
    }

    // Send CLOSE
    let close = encode_frame(FRAME_CLOSE, stream_id, &[]);
    let _ = ws_tx.send(Message::Binary(close)).await;
    streams.lock().await.remove(&stream_id);
}

/// Drain a channel of byte buffers into a TCP writer half.
async fn pump_channel_to_writer<W: tokio::io::AsyncWrite + Unpin>(
    mut writer: W,
    mut rx: mpsc::Receiver<Vec<u8>>,
    streams: StreamMap,
    stream_id: u64,
    cancel: CancellationToken,
) {
    loop {
        let next = tokio::select! {
            _ = cancel.cancelled() => None,
            next = rx.recv() => next,
        };

        let Some(payload) = next else { break };
        if writer.write_all(&payload).await.is_err() {
            break;
        }
    }

    let _ = writer.shutdown().await;
    streams.lock().await.remove(&stream_id);
}

// ═══════════════════════════════════════════════════════════════════════
//  FRAME ENCODING / DECODING
// ═══════════════════════════════════════════════════════════════════════

struct Frame<'a> {
    kind: u8,
    stream_id: u64,
    payload: &'a [u8],
}

fn encode_frame(kind: u8, stream_id: u64, payload: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 + 8 + payload.len());
    buf.push(kind);
    buf.extend_from_slice(&stream_id.to_be_bytes());
    buf.extend_from_slice(payload);
    buf
}

fn decode_frame(data: &[u8]) -> Result<Frame<'_>> {
    if data.len() < 9 {
        return Err(anyhow!("WSS relay frame too short ({} bytes)", data.len()));
    }
    let kind = data[0];
    let stream_id = u64::from_be_bytes(data[1..9].try_into().unwrap());
    let payload = &data[9..];

    // Transparent zstd decompression for FRAME_DATA_ZSTD
    if kind == FRAME_DATA_ZSTD {
        // We can't return a borrowed slice for decompressed data in Frame<'_>,
        // so we return the raw payload and let the caller decompress.
        // To keep the API simple, we re-tag as FRAME_DATA here.
        // The actual decompression happens inline in the handler.
        return Ok(Frame {
            kind: FRAME_DATA,
            stream_id,
            payload,
        });
    }

    Ok(Frame {
        kind,
        stream_id,
        payload,
    })
}

/// Encode a DATA frame with optional zstd compression.
/// If the payload is large enough and compresses well, use FRAME_DATA_ZSTD.
fn encode_data_frame_maybe_compressed(stream_id: u64, payload: &[u8]) -> Vec<u8> {
    if payload.len() < ZSTD_MIN_PAYLOAD {
        return encode_frame(FRAME_DATA, stream_id, payload);
    }

    match zstd::stream::encode_all(std::io::Cursor::new(payload), ZSTD_LEVEL) {
        Ok(compressed) if compressed.len() < payload.len() => {
            encode_frame(FRAME_DATA_ZSTD, stream_id, &compressed)
        }
        _ => encode_frame(FRAME_DATA, stream_id, payload),
    }
}

/// Decompress a zstd-encoded DATA payload.
fn zstd_decompress_data(compressed: &[u8]) -> Result<Vec<u8>> {
    zstd::stream::decode_all(std::io::Cursor::new(compressed))
        .context("zstd decompression failed for WSS relay frame")
}
