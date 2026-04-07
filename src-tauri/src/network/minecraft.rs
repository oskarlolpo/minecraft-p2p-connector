use anyhow::{anyhow, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task,
    time::{timeout, Duration},
};

use crate::models::{LanPortDetection, LocalTargetState, PreflightReport};

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

    Err(last_error.unwrap_or_else(|| anyhow!("failed to get a valid Minecraft status response")))
}

pub async fn build_preflight_report(port: u16) -> PreflightReport {
    match detect_local_version(port).await {
        Ok(version) => PreflightReport {
            local_port: port,
            reachable: true,
            state: LocalTargetState::Reachable,
            minecraft_version: Some(version),
            recommended_host_action:
                "Local Minecraft is reachable. You can launch the host and publish the room.".into(),
            note: Some("The world is already open to LAN or the local server is accepting connections.".into()),
        },
        Err(version_error) => match probe_local_tcp(port).await {
            Ok(()) => PreflightReport {
                local_port: port,
                reachable: true,
                state: LocalTargetState::Reachable,
                minecraft_version: None,
                recommended_host_action:
                    "The TCP port is reachable, but Minecraft version detection failed. You can still launch the host.".into(),
                note: Some(format!("Version detection failed during status ping: {version_error:#}")),
            },
            Err(reachability_error) => PreflightReport {
                local_port: port,
                reachable: false,
                state: LocalTargetState::Unreachable,
                minecraft_version: None,
                recommended_host_action:
                    "Open the world to LAN or start the local Minecraft server, then try hosting again.".into(),
                note: Some(format!(
                    "Local TCP reachability check failed: {reachability_error:#}; status ping: {version_error:#}"
                )),
            },
        },
    }
}

pub async fn detect_lan_port_from_logs() -> Result<LanPortDetection> {
    task::spawn_blocking(detect_lan_port_from_logs_blocking)
        .await
        .context("failed to await Minecraft LAN port detector task")?
}

async fn query_status(port: u16, protocol_version: i32) -> Result<String> {
    let target = format!("127.0.0.1:{port}");
    let mut stream = timeout(Duration::from_secs(2), TcpStream::connect(&target))
        .await
        .context("timed out while connecting to the local Minecraft target")?
        .with_context(|| format!("failed to connect to {target}"))?;

    let handshake = build_handshake_packet("127.0.0.1", port, protocol_version)?;
    stream.write_all(&handshake).await?;
    stream.write_all(&[0x01, 0x00]).await?;
    stream.flush().await?;

    let _packet_length = read_varint(&mut stream).await?;
    let packet_id = read_varint(&mut stream).await?;
    if packet_id != 0 {
        return Err(anyhow!("unexpected packet id {packet_id}"));
    }

    let payload_len = read_varint(&mut stream).await?;
    if payload_len < 0 {
        return Err(anyhow!("received a negative status payload length"));
    }

    let mut payload = vec![0u8; payload_len as usize];
    stream.read_exact(&mut payload).await?;

    let response: StatusResponse =
        serde_json::from_slice(&payload).context("failed to parse Minecraft status JSON")?;
    Ok(response.version.name)
}

async fn probe_local_tcp(port: u16) -> Result<()> {
    let target = format!("127.0.0.1:{port}");
    let stream = timeout(Duration::from_secs(2), TcpStream::connect(&target))
        .await
        .context("timed out during the local Minecraft TCP probe")?
        .with_context(|| format!("failed to connect to {target}"))?;
    stream
        .writable()
        .await
        .with_context(|| format!("local Minecraft target {target} never became writable"))?;
    Ok(())
}

fn detect_lan_port_from_logs_blocking() -> Result<LanPortDetection> {
    let candidates = collect_log_candidates()?;
    for path in candidates {
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Some(detection) = parse_lan_port_from_contents(&path, &contents) {
                return Ok(detection);
            }
        }
    }

    Err(anyhow!(
        "could not find a recent 'Started serving on <port>' entry in Minecraft logs"
    ))
}

fn collect_log_candidates() -> Result<Vec<PathBuf>> {
    let mut roots = Vec::new();

    if let Some(app_data) = std::env::var_os("APPDATA") {
        let app_data = PathBuf::from(app_data);
        roots.push(app_data.join(".minecraft").join("logs"));
        roots.push(app_data.join("PrismLauncher").join("instances"));
        roots.push(app_data.join("MultiMC").join("instances"));
    }

    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let user_profile = PathBuf::from(user_profile);
        roots.push(
            user_profile
                .join("curseforge")
                .join("minecraft")
                .join("Instances"),
        );
    }

    let mut candidates = Vec::new();
    for root in roots {
        push_candidate_logs(&root, 0, &mut candidates);
    }

    candidates.sort_by(|left, right| {
        let left_time = file_modified(left);
        let right_time = file_modified(right);
        right_time.cmp(&left_time)
    });
    candidates.dedup();
    Ok(candidates)
}

fn push_candidate_logs(root: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > 4 || !root.exists() {
        return;
    }

    if root.is_file() {
        if root
            .file_name()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("latest.log"))
            .unwrap_or(false)
        {
            out.push(root.to_path_buf());
        }
        return;
    }

    if let Some(name) = root.file_name().and_then(|value| value.to_str()) {
        if name.eq_ignore_ascii_case("logs") {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_file()
                        && path
                            .extension()
                            .and_then(|value| value.to_str())
                            .map(|value| value.eq_ignore_ascii_case("log"))
                            .unwrap_or(false)
                    {
                        out.push(path);
                    }
                }
            }
            return;
        }
    }

    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(Result::ok) {
            push_candidate_logs(&entry.path(), depth + 1, out);
        }
    }
}

fn file_modified(path: &Path) -> std::time::SystemTime {
    fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

fn parse_lan_port_from_contents(path: &Path, contents: &str) -> Option<LanPortDetection> {
    for line in contents.lines().rev() {
        if let Some(port) = extract_port_from_line(line) {
            return Some(LanPortDetection {
                port,
                source_path: path.display().to_string(),
                source_line: line.trim().to_string(),
            });
        }
    }
    None
}

fn extract_port_from_line(line: &str) -> Option<u16> {
    let marker = "Started serving on ";
    let index = line.find(marker)?;
    let port_part = &line[index + marker.len()..];
    let digits = port_part
        .chars()
        .take_while(|value| value.is_ascii_digit())
        .collect::<String>();
    if digits.is_empty() {
        return None;
    }
    digits.parse().ok()
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
    let mut value = u32::try_from(value).context("negative VarInt is not supported")?;
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
            return Err(anyhow!("VarInt is too long"));
        }

        let byte = stream.read_u8().await?;
        value |= i32::from(byte & 0x7F) << position;

        if byte & 0x80 == 0 {
            return Ok(value);
        }

        position += 7;
    }
}
