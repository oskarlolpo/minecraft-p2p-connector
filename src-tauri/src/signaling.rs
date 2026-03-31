use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use tokio::{net::UdpSocket, time::timeout};
use tokio_util::sync::CancellationToken;

const DEFAULT_WS_URL: &str = "ws://127.0.0.1:9001/ws";
const DEFAULT_UDP_ADDR: &str = "127.0.0.1:9002";

#[derive(Debug, Clone)]
pub struct SignalingConfig {
    pub ws_url: String,
    pub udp_addr: SocketAddr,
}

impl SignalingConfig {
    pub fn from_env() -> Self {
        let ws_url = std::env::var("MC_SIGNAL_WS_URL").unwrap_or_else(|_| DEFAULT_WS_URL.into());
        let udp_addr = std::env::var("MC_SIGNAL_UDP_ADDR")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or_else(|| {
                DEFAULT_UDP_ADDR
                    .parse()
                    .expect("default UDP address must be valid")
            });

        Self { ws_url, udp_addr }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientSignal {
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
pub enum ServerSignal {
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

pub async fn register_udp_mapping(
    socket: Arc<UdpSocket>,
    config: &SignalingConfig,
    token: &str,
) -> Result<SocketAddr> {
    let request = serde_json::to_vec(&UdpRegistration {
        token: token.to_owned(),
    })?;
    let mut buffer = [0u8; 256];

    for _ in 0..12 {
        socket.send_to(&request, config.udp_addr).await?;

        if let Ok(Ok((size, _))) =
            timeout(Duration::from_millis(300), socket.recv_from(&mut buffer)).await
        {
            let ack: UdpAck = serde_json::from_slice(&buffer[..size])?;
            if ack.ok && ack.token == token {
                return Ok(ack.observed_addr.parse()?);
            }
        }
    }

    Err(anyhow!("signaling UDP registration timed out"))
}

pub async fn punch_remote(
    socket: Arc<UdpSocket>,
    remote: SocketAddr,
    room_code: &str,
    peer_id: &str,
    cancel: CancellationToken,
) -> Result<()> {
    let payload = format!("MCP2P-PUNCH|{room_code}|{peer_id}");
    for _ in 0..32 {
        if cancel.is_cancelled() {
            break;
        }
        socket.send_to(payload.as_bytes(), remote).await?;
        tokio::time::sleep(Duration::from_millis(75)).await;
    }

    Ok(())
}
