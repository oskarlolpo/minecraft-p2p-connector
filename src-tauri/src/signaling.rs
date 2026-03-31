use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, QoS};
use serde::{Deserialize, Serialize};
use tokio::{
    net::{lookup_host, UdpSocket},
    sync::mpsc,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

const DEFAULT_MQTT_HOST: &str = "test.mosquitto.org";
const DEFAULT_MQTT_PORT: u16 = 1883;
const DEFAULT_STUN_SERVERS: &str =
    "stun.cloudflare.com:3478,stun.l.google.com:19302,stun1.l.google.com:19302";

#[derive(Debug, Clone)]
pub struct SignalingConfig {
    pub mqtt_host: String,
    pub mqtt_port: u16,
    pub stun_servers: Vec<String>,
}

impl SignalingConfig {
    pub fn from_env() -> Self {
        let mqtt_host =
            std::env::var("MC_SIGNAL_MQTT_HOST").unwrap_or_else(|_| DEFAULT_MQTT_HOST.into());
        let mqtt_port = std::env::var("MC_SIGNAL_MQTT_PORT")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_MQTT_PORT);
        let stun_servers = std::env::var("MC_STUN_SERVERS")
            .unwrap_or_else(|_| DEFAULT_STUN_SERVERS.into())
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        Self {
            mqtt_host,
            mqtt_port,
            stun_servers,
        }
    }

    pub fn broker_label(&self) -> String {
        format!("mqtt://{}:{}", self.mqtt_host, self.mqtt_port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrokerMessage {
    HostOffer {
        room_code: String,
        peer_id: String,
        peer_addr: String,
        peer_cert: String,
    },
    JoinRequest {
        room_code: String,
        peer_id: String,
        peer_addr: String,
    },
}

pub struct BrokerConnection {
    pub client: AsyncClient,
    pub receiver: mpsc::UnboundedReceiver<BrokerMessage>,
}

impl BrokerConnection {
    pub async fn connect(config: &SignalingConfig, client_id: &str) -> Result<Self> {
        let mut mqtt_options = MqttOptions::new(client_id, &config.mqtt_host, config.mqtt_port);
        mqtt_options.set_keep_alive(Duration::from_secs(5));
        mqtt_options.set_clean_session(true);
        mqtt_options.set_max_packet_size(256 * 1024, 256 * 1024);

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, 20);
        let (tx, rx) = mpsc::unbounded_channel();

        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(Incoming::Publish(publish))) => {
                        if let Ok(message) =
                            serde_json::from_slice::<BrokerMessage>(&publish.payload)
                        {
                            let _ = tx.send(message);
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                    }
                }
            }
        });

        Ok(Self {
            client,
            receiver: rx,
        })
    }
}

pub fn room_offer_topic(room_code: &str) -> String {
    format!("minecraft-p2p-connector/v1/{room_code}/offer")
}

pub fn room_join_topic(room_code: &str) -> String {
    format!("minecraft-p2p-connector/v1/{room_code}/join")
}

pub fn new_room_code() -> String {
    Uuid::new_v4().simple().to_string()[..6].to_uppercase()
}

pub async fn publish_broker_message(
    client: &AsyncClient,
    topic: &str,
    retain: bool,
    message: &BrokerMessage,
) -> Result<()> {
    client
        .publish(
            topic,
            QoS::AtLeastOnce,
            retain,
            serde_json::to_vec(message)?,
        )
        .await?;
    Ok(())
}

pub async fn subscribe_topic(client: &AsyncClient, topic: &str) -> Result<()> {
    client.subscribe(topic, QoS::AtLeastOnce).await?;
    Ok(())
}

pub async fn discover_public_addr(
    socket: Arc<UdpSocket>,
    config: &SignalingConfig,
) -> Result<SocketAddr> {
    for server in &config.stun_servers {
        if let Ok(addr) = discover_public_addr_via_stun(socket.clone(), server).await {
            return Ok(addr);
        }
    }

    Err(anyhow!("failed to discover public UDP address via STUN"))
}

async fn discover_public_addr_via_stun(socket: Arc<UdpSocket>, server: &str) -> Result<SocketAddr> {
    let request = build_stun_binding_request();
    let mut buffer = [0u8; 1024];
    let server_addr = lookup_host(server)
        .await
        .with_context(|| format!("failed to resolve STUN server: {server}"))?
        .find(SocketAddr::is_ipv4)
        .ok_or_else(|| anyhow!("no IPv4 STUN address resolved for {server}"))?;

    for _ in 0..4 {
        socket.send_to(&request, server_addr).await?;
        if let Ok(Ok((size, _))) =
            timeout(Duration::from_millis(900), socket.recv_from(&mut buffer)).await
        {
            if let Ok(addr) = parse_stun_binding_response(&request, &buffer[..size]) {
                return Ok(addr);
            }
        }
    }

    Err(anyhow!("STUN request to {server} timed out"))
}

fn build_stun_binding_request() -> [u8; 20] {
    let mut request = [0u8; 20];
    request[0] = 0x00;
    request[1] = 0x01;
    request[4..8].copy_from_slice(&0x2112_A442u32.to_be_bytes());

    let tx_id = Uuid::new_v4();
    request[8..20].copy_from_slice(&tx_id.as_bytes()[..12]);
    request
}

fn parse_stun_binding_response(request: &[u8; 20], payload: &[u8]) -> Result<SocketAddr> {
    if payload.len() < 20 {
        return Err(anyhow!("short STUN response"));
    }

    let msg_type = u16::from_be_bytes([payload[0], payload[1]]);
    if msg_type != 0x0101 {
        return Err(anyhow!("unexpected STUN response type"));
    }
    if payload[8..20] != request[8..20] {
        return Err(anyhow!("STUN transaction id mismatch"));
    }

    let mut offset = 20usize;
    while offset + 4 <= payload.len() {
        let attr_type = u16::from_be_bytes([payload[offset], payload[offset + 1]]);
        let attr_len = u16::from_be_bytes([payload[offset + 2], payload[offset + 3]]) as usize;
        let value_start = offset + 4;
        let value_end = value_start + attr_len;
        if value_end > payload.len() {
            break;
        }

        match attr_type {
            0x0020 => {
                if let Some(addr) = parse_xor_mapped_address(&payload[value_start..value_end]) {
                    return Ok(addr);
                }
            }
            0x0001 => {
                if let Some(addr) = parse_mapped_address(&payload[value_start..value_end]) {
                    return Ok(addr);
                }
            }
            _ => {}
        }

        offset = value_end.next_multiple_of(4);
    }

    Err(anyhow!("no mapped address in STUN response"))
}

fn parse_xor_mapped_address(value: &[u8]) -> Option<SocketAddr> {
    if value.len() < 8 || value[1] != 0x01 {
        return None;
    }

    let port = u16::from_be_bytes([value[2], value[3]]) ^ ((0x2112_A442u32 >> 16) as u16);
    let cookie = 0x2112_A442u32.to_be_bytes();
    let ip = std::net::Ipv4Addr::new(
        value[4] ^ cookie[0],
        value[5] ^ cookie[1],
        value[6] ^ cookie[2],
        value[7] ^ cookie[3],
    );
    Some(SocketAddr::new(ip.into(), port))
}

fn parse_mapped_address(value: &[u8]) -> Option<SocketAddr> {
    if value.len() < 8 || value[1] != 0x01 {
        return None;
    }
    let port = u16::from_be_bytes([value[2], value[3]]);
    let ip = std::net::Ipv4Addr::new(value[4], value[5], value[6], value[7]);
    Some(SocketAddr::new(ip.into(), port))
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
