use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    time::{timeout, Duration},
};

use crate::models::{LocalTargetState, PreflightReport};

const STATUS_PROTOCOL_CANDIDATES: &[i32] = &[767, 764, 760, 47];

#[derive(Debug, Deserialize)]
struct StatusResponse {
    version: MinecraftVersion,
}

#[derive(Debug, Deserialize)]
struct MinecraftVersion {
    name: String,
}

pub async fn detect_local_version(port: u16) -> Result<String> {
    let mut last_error = None;
    for protocol_version in STATUS_PROTOCOL_CANDIDATES {
        match query_status(port, *protocol_version).await {
            Ok(version) => return Ok(version),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("не удалось получить ответ status ping")))
}

pub async fn build_preflight_report(port: u16) -> PreflightReport {
    match detect_local_version(port).await {
        Ok(version) => PreflightReport {
            local_port: port,
            reachable: true,
            state: LocalTargetState::Reachable,
            minecraft_version: Some(version),
            recommended_host_action:
                "Локальный Minecraft отвечает. Можно запускать хост и публиковать комнату.".into(),
            note: Some(
                "Мир уже открыт в LAN или локальный сервер принимает подключения.".into(),
            ),
        },
        Err(version_error) => match probe_local_tcp(port).await {
            Ok(()) => PreflightReport {
                local_port: port,
                reachable: true,
                state: LocalTargetState::Reachable,
                minecraft_version: None,
                recommended_host_action:
                    "TCP-порт доступен, но версия не определилась. Хост можно запускать, но стоит проверить совместимость клиента.".into(),
                note: Some(format!(
                    "Status ping не смог определить версию: {version_error:#}"
                )),
            },
            Err(reachability_error) => PreflightReport {
                local_port: port,
                reachable: false,
                state: LocalTargetState::Unreachable,
                minecraft_version: None,
                recommended_host_action:
                    "Сначала откройте мир в LAN или запустите локальный Minecraft сервер, затем повторите запуск хоста.".into(),
                note: Some(format!(
                    "Локальный TCP check не прошёл: {reachability_error:#}; status ping: {version_error:#}"
                )),
            },
        },
    }
}

async fn query_status(port: u16, protocol_version: i32) -> Result<String> {
    let target = format!("127.0.0.1:{port}");
    let mut stream = timeout(Duration::from_secs(2), TcpStream::connect(&target))
        .await
        .context("таймаут подключения к локальному Minecraft серверу")?
        .with_context(|| format!("не удалось подключиться к {target}"))?;

    let handshake = build_handshake_packet("127.0.0.1", port, protocol_version)?;
    stream.write_all(&handshake).await?;
    stream.write_all(&[0x01, 0x00]).await?;
    stream.flush().await?;

    let _packet_length = read_varint(&mut stream).await?;
    let packet_id = read_varint(&mut stream).await?;
    if packet_id != 0 {
        return Err(anyhow!("получен неожиданный packet id {packet_id}"));
    }

    let payload_len = read_varint(&mut stream).await?;
    if payload_len < 0 {
        return Err(anyhow!("получена отрицательная длина ответа"));
    }

    let mut payload = vec![0u8; payload_len as usize];
    stream.read_exact(&mut payload).await?;

    let response: StatusResponse =
        serde_json::from_slice(&payload).context("не удалось распарсить JSON status ping")?;
    Ok(response.version.name)
}

async fn probe_local_tcp(port: u16) -> Result<()> {
    let target = format!("127.0.0.1:{port}");
    let stream = timeout(Duration::from_secs(2), TcpStream::connect(&target))
        .await
        .context("таймаут при TCP-проверке локального Minecraft")?
        .with_context(|| format!("не удалось подключиться к {target}"))?;
    stream
        .writable()
        .await
        .with_context(|| format!("локальный Minecraft на {target} не стал writable"))?;
    Ok(())
}

fn build_handshake_packet(host: &str, port: u16, protocol_version: i32) -> Result<Vec<u8>> {
    let mut packet = Vec::new();
    packet.push(0x00);
    write_varint(&mut packet, protocol_version)?;
    write_varint(&mut packet, host.len() as i32)?;
    packet.extend_from_slice(host.as_bytes());
    packet.extend_from_slice(&port.to_be_bytes());
    write_varint(&mut packet, 1)?;

    let mut framed = Vec::new();
    write_varint(&mut framed, packet.len() as i32)?;
    framed.extend_from_slice(&packet);
    Ok(framed)
}

fn write_varint(buffer: &mut Vec<u8>, value: i32) -> Result<()> {
    let mut value = u32::try_from(value).context("отрицательный VarInt не поддерживается")?;
    loop {
        if value & !0x7F == 0 {
            buffer.push(value as u8);
            return Ok(());
        }

        buffer.push(((value & 0x7F) | 0x80) as u8);
        value >>= 7;
    }
}

async fn read_varint(stream: &mut TcpStream) -> Result<i32> {
    let mut value = 0i32;
    let mut position = 0;

    loop {
        if position >= 35 {
            return Err(anyhow!("VarInt слишком длинный"));
        }

        let byte = stream.read_u8().await?;
        value |= i32::from(byte & 0x7F) << position;

        if byte & 0x80 == 0 {
            return Ok(value);
        }

        position += 7;
    }
}
