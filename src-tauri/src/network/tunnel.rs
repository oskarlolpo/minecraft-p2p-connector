use std::{io::ErrorKind, time::Duration};

use anyhow::{anyhow, bail, Context, Result};
use futures::{SinkExt, StreamExt};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use socket2::{SockRef, TcpKeepalive};
use tokio::{
    io::{AsyncRead, AsyncWrite, AsyncWriteExt},
    net::TcpStream,
    task::JoinHandle,
    time::timeout,
};
use tokio_util::{
    codec::{AnyDelimiterCodec, Framed, FramedParts},
    sync::CancellationToken,
};
use uuid::Uuid;

pub const DEFAULT_BORE_HOST: &str = "bore.pub";
pub const DEFAULT_BORE_CONTROL_PORT: u16 = 7835;
const FRAME_MAX_LENGTH: usize = 256;
const CONTROL_TIMEOUT: Duration = Duration::from_secs(5);
const BRIDGE_CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
const LOCAL_TARGET_RETRY_DELAY: Duration = Duration::from_millis(350);
const LOCAL_TARGET_RETRY_ATTEMPTS: usize = 5;

#[derive(Debug, Clone)]
pub struct ReverseTunnelConfig {
    pub server_host: String,
    pub control_port: u16,
    pub requested_remote_port: u16,
    pub local_host: String,
    pub local_port: u16,
}

impl ReverseTunnelConfig {
    pub fn bore_pub(local_port: u16) -> Self {
        Self {
            server_host: DEFAULT_BORE_HOST.into(),
            control_port: DEFAULT_BORE_CONTROL_PORT,
            requested_remote_port: 0,
            local_host: "127.0.0.1".into(),
            local_port,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ReverseTunnelEndpoint {
    pub public_host: String,
    pub public_port: u16,
}

impl ReverseTunnelEndpoint {
    pub fn as_socket_label(&self) -> String {
        format!("{}:{}", self.public_host, self.public_port)
    }

    pub fn as_multiaddr(&self) -> String {
        if self.public_host.parse::<std::net::Ipv4Addr>().is_ok() {
            format!("/ip4/{}/tcp/{}", self.public_host, self.public_port)
        } else if self.public_host.parse::<std::net::Ipv6Addr>().is_ok() {
            format!("/ip6/{}/tcp/{}", self.public_host, self.public_port)
        } else {
            format!("/dns4/{}/tcp/{}", self.public_host, self.public_port)
        }
    }
}

pub struct ReverseTunnelHandle {
    endpoint: ReverseTunnelEndpoint,
    task: JoinHandle<Result<()>>,
}

impl ReverseTunnelHandle {
    pub fn endpoint(&self) -> &ReverseTunnelEndpoint {
        &self.endpoint
    }

    pub fn abort(&self) {
        self.task.abort();
    }
}

#[derive(Debug, Serialize, Deserialize)]
enum ClientMessage {
    Hello(u16),
    Accept(Uuid),
}

#[derive(Debug, Serialize, Deserialize)]
enum ServerMessage {
    Hello(u16),
    Heartbeat,
    Connection(Uuid),
    Error(String),
}

pub async fn start_reverse_tunnel(
    config: ReverseTunnelConfig,
    cancel: CancellationToken,
) -> Result<ReverseTunnelHandle> {
    let mut control = Delimited::new(connect_with_timeout(&config.server_host, config.control_port).await?);
    control
        .send(ClientMessage::Hello(config.requested_remote_port))
        .await
        .context("не удалось отправить Hello на reverse tunnel server")?;

    let remote_port = match control.recv_timeout::<ServerMessage>().await? {
        Some(ServerMessage::Hello(port)) => port,
        Some(ServerMessage::Error(message)) => bail!("reverse tunnel server error: {message}"),
        Some(other) => bail!("неожиданный ответ reverse tunnel server: {other:?}"),
        None => bail!("reverse tunnel server закрыл control connection"),
    };

    let endpoint = ReverseTunnelEndpoint {
        public_host: config.server_host.clone(),
        public_port: remote_port,
    };

    let task = tokio::spawn(run_reverse_tunnel_control_loop(control, config, cancel));

    Ok(ReverseTunnelHandle { endpoint, task })
}

pub async fn bridge_tcp_to_remote(
    mut local_stream: TcpStream,
    remote_host: &str,
    remote_port: u16,
) -> Result<()> {
    configure_tcp_stream(&local_stream)
        .with_context(|| format!("не удалось настроить локальный TCP stream для {remote_host}:{remote_port}"))?;

    let mut remote_stream = connect_with_timeout(remote_host, remote_port)
        .await
        .with_context(|| format!("не удалось подключиться к reverse tunnel endpoint {remote_host}:{remote_port}"))?;
    configure_tcp_stream(&remote_stream)
        .with_context(|| format!("не удалось настроить reverse tunnel stream для {remote_host}:{remote_port}"))?;

    copy_bidirectional_tolerant(&mut local_stream, &mut remote_stream)
        .await
        .context("copy_bidirectional через reverse tunnel завершился ошибкой")?;
    Ok(())
}

async fn run_reverse_tunnel_control_loop(
    mut control: Delimited<TcpStream>,
    config: ReverseTunnelConfig,
    cancel: CancellationToken,
) -> Result<()> {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            frame = control.recv::<ServerMessage>() => {
                match frame? {
                    Some(ServerMessage::Heartbeat) => {}
                    Some(ServerMessage::Connection(id)) => {
                        let connection_config = config.clone();
                        tokio::spawn(async move {
                            if let Err(error) = accept_reverse_connection(connection_config, id).await {
                                tracing::warn!("reverse tunnel connection {id} failed: {error:#}");
                            }
                        });
                    }
                    Some(ServerMessage::Error(message)) => return Err(anyhow!("reverse tunnel server error: {message}")),
                    Some(ServerMessage::Hello(_)) => {
                        tracing::debug!("ignoring duplicate Hello from reverse tunnel server");
                    }
                    None => break,
                }
            }
        }
    }

    Ok(())
}

async fn accept_reverse_connection(config: ReverseTunnelConfig, id: Uuid) -> Result<()> {
    let mut remote = Delimited::new(connect_with_timeout(&config.server_host, config.control_port).await?);
    remote
        .send(ClientMessage::Accept(id))
        .await
        .context("не удалось отправить Accept на reverse tunnel server")?;

    let mut local = connect_local_target_with_retry(&config.local_host, config.local_port)
        .await
        .with_context(|| format!("не удалось подключиться к локальному target {}:{}", config.local_host, config.local_port))?;
    configure_tcp_stream(&local)
        .with_context(|| format!("не удалось настроить локальный target {}:{}", config.local_host, config.local_port))?;

    let mut parts = remote.into_parts();
    if !parts.read_buf.is_empty() {
        local
            .write_all(parts.read_buf.as_ref())
            .await
            .context("не удалось отправить buffered bytes в локальный target")?;
    }

    copy_bidirectional_tolerant(&mut local, &mut parts.io)
        .await
        .context("copy_bidirectional reverse tunnel завершился ошибкой")?;
    Ok(())
}

async fn connect_with_timeout(host: &str, port: u16) -> Result<TcpStream> {
    let stream = timeout(BRIDGE_CONNECT_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .context("таймаут TCP connect")?
        .with_context(|| format!("не удалось подключиться к {host}:{port}"))?;
    configure_tcp_stream(&stream)
        .with_context(|| format!("не удалось настроить TCP stream для {host}:{port}"))?;
    Ok(stream)
}

async fn connect_local_target_with_retry(host: &str, port: u16) -> Result<TcpStream> {
    let mut last_error = None;

    for attempt in 1..=LOCAL_TARGET_RETRY_ATTEMPTS {
        match connect_with_timeout(host, port).await {
            Ok(stream) => return Ok(stream),
            Err(error) => {
                tracing::warn!(
                    "local target {}:{} is not ready on attempt {}/{}: {error:#}",
                    host,
                    port,
                    attempt,
                    LOCAL_TARGET_RETRY_ATTEMPTS
                );
                last_error = Some(error);
                if attempt < LOCAL_TARGET_RETRY_ATTEMPTS {
                    tokio::time::sleep(LOCAL_TARGET_RETRY_DELAY).await;
                }
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("локальный target {host}:{port} недоступен")))
}

fn configure_tcp_stream(stream: &TcpStream) -> Result<()> {
    stream.set_nodelay(true).context("set_nodelay failed")?;
    let keepalive = TcpKeepalive::new()
        .with_time(Duration::from_secs(20))
        .with_interval(Duration::from_secs(10));
    let socket = SockRef::from(stream);
    socket.set_tcp_keepalive(&keepalive).context("set_tcp_keepalive failed")?;
    Ok(())
}

async fn copy_bidirectional_tolerant<A, B>(left: &mut A, right: &mut B) -> Result<()>
where
    A: AsyncRead + AsyncWrite + Unpin,
    B: AsyncRead + AsyncWrite + Unpin,
{
    match tokio::io::copy_bidirectional(left, right).await {
        Ok(_) => Ok(()),
        Err(error) if is_connection_close(&error) => {
            tracing::debug!("reverse tunnel bridge closed by peer: {error}");
            Ok(())
        }
        Err(error) => Err(error).context("tokio::io::copy_bidirectional вернул ошибку"),
    }
}

fn is_connection_close(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        ErrorKind::ConnectionReset | ErrorKind::BrokenPipe | ErrorKind::UnexpectedEof | ErrorKind::ConnectionAborted
    )
}

struct Delimited<U>(Framed<U, AnyDelimiterCodec>);

impl<U: AsyncRead + AsyncWrite + Unpin> Delimited<U> {
    fn new(stream: U) -> Self {
        let codec = AnyDelimiterCodec::new_with_max_length(vec![0], vec![0], FRAME_MAX_LENGTH);
        Self(Framed::new(stream, codec))
    }

    async fn recv<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        match self.0.next().await {
            Some(frame) => {
                let bytes = frame.context("ошибка frame reverse tunnel protocol")?;
                Ok(Some(
                    serde_json::from_slice(&bytes).context("не удалось распарсить JSON control frame")?,
                ))
            }
            None => Ok(None),
        }
    }

    async fn recv_timeout<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        timeout(CONTROL_TIMEOUT, self.recv())
            .await
            .context("таймаут ожидания control frame")?
    }

    async fn send<T: Serialize>(&mut self, msg: T) -> Result<()> {
        self.0
            .send(serde_json::to_string(&msg)?)
            .await
            .context("не удалось отправить JSON control frame")?;
        Ok(())
    }

    fn into_parts(self) -> FramedParts<U, AnyDelimiterCodec> {
        self.0.into_parts()
    }
}
