use std::{
    net::{SocketAddr, ToSocketAddrs},
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, bail, Context, Result};
use quinn::{ClientConfig, Connection, Endpoint, EndpointConfig, RecvStream, SendStream};
use serde::Deserialize;
use serde_json::json;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

use crate::cert::build_insecure_client_config_with_alpn;

#[derive(Clone)]
pub struct E4mcConfig {
    pub enabled_by_default: bool,
    pub use_broker: bool,
    pub broker_url: String,
    pub relay_host: String,
    pub relay_port: u16,
}

pub struct E4mcRuntime {
    pub domain: String,
    task: JoinHandle<Result<()>>,
}

#[derive(Debug, Clone)]
struct RelayTarget {
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct BrokerResponse {
    host: String,
    port: u16,
}

#[derive(Debug, Deserialize)]
struct ControlEnvelope {
    kind: String,
    domain: Option<String>,
    message: Option<String>,
}

const QUICLIME_ALPN: &[u8] = b"quiclime";
const CONTROL_TIMEOUT: Duration = Duration::from_secs(15);

impl E4mcConfig {
    pub fn from_env() -> Self {
        Self {
            enabled_by_default: std::env::var("MC_E4MC_ENABLED")
                .ok()
                .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
                .unwrap_or(true),
            use_broker: std::env::var("MC_E4MC_USE_BROKER")
                .ok()
                .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
                .unwrap_or(true),
            broker_url: std::env::var("MC_E4MC_BROKER_URL")
                .unwrap_or_else(|_| "https://broker.e4mc.link/getBestRelay".into()),
            relay_host: std::env::var("MC_E4MC_RELAY_HOST")
                .unwrap_or_else(|_| "test.e4mc.link".into()),
            relay_port: std::env::var("MC_E4MC_RELAY_PORT")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(25575),
        }
    }
}

impl E4mcRuntime {
    pub async fn wait(self) -> Result<()> {
        self.task.await.context("e4mc background task panicked")?
    }
}

pub async fn start_host_runtime(
    config: E4mcConfig,
    local_port: u16,
    cancel: CancellationToken,
) -> Result<E4mcRuntime> {
    let relay = resolve_relay(config).await?;
    let remote_addr = resolve_socket_addr(&relay.host, relay.port)?;

    let std_socket = std::net::UdpSocket::bind("0.0.0.0:0")
        .context("failed to bind UDP socket for e4mc session")?;
    std_socket
        .set_nonblocking(true)
        .context("failed to mark e4mc UDP socket as non-blocking")?;

    let mut endpoint = Endpoint::new(
        EndpointConfig::default(),
        None,
        std_socket,
        Arc::new(quinn::TokioRuntime),
    )
    .context("failed to create e4mc QUIC endpoint")?;
    endpoint.set_default_client_config(build_quiclime_client_config()?);

    let connection = endpoint
        .connect(remote_addr, &relay.host)
        .with_context(|| format!("failed to start e4mc QUIC connect to {}:{}", relay.host, relay.port))?
        .await
        .with_context(|| format!("failed to establish e4mc QUIC session to {}:{}", relay.host, relay.port))?;

    let (domain, control_send, control_recv) = request_domain_assignment(connection.clone())
        .await
        .context("e4mc control channel did not return a domain")?;

    let task = tokio::spawn(run_e4mc_host_loop(
        endpoint,
        connection,
        control_send,
        control_recv,
        local_port,
        cancel,
    ));

    Ok(E4mcRuntime { domain, task })
}

async fn resolve_relay(config: E4mcConfig) -> Result<RelayTarget> {
    if config.use_broker {
        let client = reqwest::Client::builder()
            .user_agent("minecraft-p2p-connector")
            .build()?;
        let response = client
            .get(&config.broker_url)
            .header("Accept", "application/json")
            .send()
            .await
            .with_context(|| format!("failed to fetch e4mc broker {}", config.broker_url))?
            .error_for_status()
            .with_context(|| format!("e4mc broker returned an error for {}", config.broker_url))?;
        let broker: BrokerResponse = response
            .json()
            .await
            .context("failed to parse e4mc broker response")?;
        return Ok(RelayTarget {
            host: broker.host,
            port: broker.port,
        });
    }

    Ok(RelayTarget {
        host: config.relay_host,
        port: config.relay_port,
    })
}

fn resolve_socket_addr(host: &str, port: u16) -> Result<SocketAddr> {
    (host, port)
        .to_socket_addrs()
        .with_context(|| format!("failed to resolve e4mc relay {host}:{port}"))?
        .next()
        .ok_or_else(|| anyhow!("e4mc relay {host}:{port} resolved to no socket addresses"))
}

fn build_quiclime_client_config() -> Result<ClientConfig> {
    build_insecure_client_config_with_alpn(&[QUICLIME_ALPN.to_vec()])
}

async fn request_domain_assignment(connection: Connection) -> Result<(String, SendStream, RecvStream)> {
    let (mut send, mut recv) = connection
        .open_bi()
        .await
        .context("failed to open e4mc control stream")?;

    send_control_message(&mut send, &json!({ "kind": "probe_capabilities" })).await?;
    send_control_message(&mut send, &json!({ "kind": "request_domain_assignment" })).await?;

    let (domain, mut recv) = timeout(CONTROL_TIMEOUT, async move {
        loop {
            let envelope = recv_control_message(&mut recv).await?;
            match envelope.kind.as_str() {
                "domain_assignment_complete" => {
                    if let Some(domain) = envelope.domain.filter(|value| !value.trim().is_empty()) {
                        return Ok((domain, recv));
                    }
                    bail!("e4mc returned an empty domain");
                }
                "request_message_broadcast" => {
                    tracing::info!(
                        "e4mc relay broadcast: {}",
                        envelope.message.unwrap_or_else(|| "<empty>".into())
                    );
                }
                "has_capabilities" | "ticket_registered" | "unknown_message" => {}
                other => {
                    tracing::debug!("ignoring unsupported e4mc control message kind: {other}");
                }
            }
        }
    })
    .await
    .context("timed out while waiting for e4mc domain assignment")??;

    Ok((domain, send, recv))
}

async fn send_control_message(send: &mut SendStream, value: &serde_json::Value) -> Result<()> {
    let payload = serde_json::to_vec(value).context("failed to encode e4mc control message")?;
    if payload.len() > 0x7f {
        bail!("e4mc control frame is unexpectedly large");
    }
    send.write_u8(payload.len() as u8)
        .await
        .context("failed to write e4mc control frame length")?;
    send.write_all(&payload)
        .await
        .context("failed to write e4mc control frame payload")?;
    send.flush().await.context("failed to flush e4mc control stream")?;
    Ok(())
}

async fn recv_control_message(recv: &mut RecvStream) -> Result<ControlEnvelope> {
    let size = recv
        .read_u8()
        .await
        .context("failed to read e4mc control frame length")?;
    let mut payload = vec![0_u8; usize::from(size)];
    recv.read_exact(&mut payload)
        .await
        .context("failed to read e4mc control frame payload")?;
    serde_json::from_slice(&payload).context("failed to decode e4mc control JSON")
}

async fn run_e4mc_host_loop(
    _endpoint: Endpoint,
    connection: Connection,
    mut control_send: SendStream,
    mut control_recv: RecvStream,
    local_port: u16,
    cancel: CancellationToken,
) -> Result<()> {
    loop {
        let incoming = tokio::select! {
            _ = cancel.cancelled() => {
                connection.close(0_u32.into(), b"session-stopped");
                return Ok(());
            }
            // keep-alive ping
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                let _ = send_control_message(&mut control_send, &json!({ "kind": "ping" })).await;
                continue;
            }
            incoming = connection.accept_bi() => incoming,
        };

        match incoming {
            Ok((send, recv)) => {
                tokio::spawn(async move {
                    if let Err(error) = bridge_raw_quic_to_local_minecraft(send, recv, local_port).await {
                        tracing::warn!("e4mc inbound stream bridge failed: {error:#}");
                    }
                });
            }
            Err(error) => {
                if cancel.is_cancelled() {
                    return Ok(());
                }
                return Err(anyhow!(error)).context("e4mc inbound stream accept failed");
            }
        }
    }
}

async fn bridge_raw_quic_to_local_minecraft(
    mut send: SendStream,
    mut recv: RecvStream,
    local_port: u16,
) -> Result<()> {
    let target_addr = format!("127.0.0.1:{local_port}");
    let minecraft_stream = TcpStream::connect(&target_addr)
        .await
        .with_context(|| format!("failed to connect to local Minecraft at {target_addr}"))?;
    let (mut minecraft_read, mut minecraft_write) = minecraft_stream.into_split();

    let uplink = async {
        tokio::io::copy(&mut minecraft_read, &mut send).await?;
        send.finish()?;
        Result::<()>::Ok(())
    };

    let downlink = async {
        tokio::io::copy(&mut recv, &mut minecraft_write).await?;
        minecraft_write.shutdown().await?;
        Result::<()>::Ok(())
    };

    tokio::try_join!(uplink, downlink)?;
    Ok(())
}
