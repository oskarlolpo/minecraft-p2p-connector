use anyhow::{anyhow, Result};
use quinn::{RecvStream, SendStream};
use tokio::{io::AsyncWriteExt, net::TcpStream};

pub const MINECRAFT_LOCAL_ADDR: &str = "127.0.0.1:25565";
pub const STREAM_MAGIC: &[u8; 7] = b"MCP2P01";

pub fn minecraft_local_addr(port: u16) -> String {
    format!("127.0.0.1:{port}")
}

pub async fn bridge_client_tcp_to_quic(
    tcp_stream: TcpStream,
    mut send: SendStream,
    mut recv: RecvStream,
) -> Result<()> {
    send.write_all(STREAM_MAGIC).await?;
    let (mut tcp_read, mut tcp_write) = tcp_stream.into_split();

    let uplink = async {
        tokio::io::copy(&mut tcp_read, &mut send).await?;
        send.finish()?;
        Result::<()>::Ok(())
    };

    let downlink = async {
        tokio::io::copy(&mut recv, &mut tcp_write).await?;
        tcp_write.shutdown().await?;
        Result::<()>::Ok(())
    };

    tokio::try_join!(uplink, downlink)?;
    Ok(())
}

pub async fn bridge_quic_to_local_minecraft(
    mut send: SendStream,
    mut recv: RecvStream,
    local_port: u16,
) -> Result<()> {
    let mut header = [0u8; STREAM_MAGIC.len()];
    recv.read_exact(&mut header).await?;
    if &header != STREAM_MAGIC {
        return Err(anyhow!("invalid QUIC stream preamble"));
    }

    let target_addr = minecraft_local_addr(local_port);
    let minecraft_stream = TcpStream::connect(&target_addr).await?;
    let (mut minecraft_read, mut minecraft_write) = minecraft_stream.into_split();

    let uplink = async {
        tokio::io::copy(&mut minecraft_read, &mut send)
            .await
            .map_err(|error| {
                tracing::warn!("host uplink copy failed for {target_addr}: {error:#}");
                error
            })?;
        send.finish()?;
        Result::<()>::Ok(())
    };

    let downlink = async {
        tokio::io::copy(&mut recv, &mut minecraft_write)
            .await
            .map_err(|error| {
                tracing::warn!("host downlink copy failed for {target_addr}: {error:#}");
                error
            })?;
        minecraft_write.shutdown().await?;
        Result::<()>::Ok(())
    };

    tokio::try_join!(uplink, downlink)?;
    Ok(())
}
