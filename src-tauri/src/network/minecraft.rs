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

use crate::models::{ExternalServerProbe, LanPortDetection, LocalTargetState, MinecraftNicknameDetection, PreflightReport};

const STATUS_PROTOCOL_CANDIDATES: &[i32] = &[767, 764, 760, 47];

#[derive(Debug, Deserialize)]
struct StatusResponse {
    version: MinecraftVersion,
    players: Option<MinecraftPlayers>,
    description: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct MinecraftVersion {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MinecraftPlayers {
    online: u32,
    max: u32,
}

pub async fn detect_local_version(port: u16) -> Result<String> {
    let mut last_error = None;
    for protocol_version in STATUS_PROTOCOL_CANDIDATES {
        match query_status("127.0.0.1", port, *protocol_version).await {
            Ok(response) => return Ok(response.version.name),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("failed to get a valid Minecraft status response")))
}

pub async fn probe_external_server(host: String, port: u16) -> Result<ExternalServerProbe> {
    let start = std::time::Instant::now();
    let mut last_error = None;
    for protocol_version in STATUS_PROTOCOL_CANDIDATES {
        match query_status(&host, port, *protocol_version).await {
            Ok(response) => {
                let players = response.players.unwrap_or(MinecraftPlayers { online: 0, max: 0 });
                return Ok(ExternalServerProbe {
                    room_name: status_description_to_string(&response.description)
                        .unwrap_or_else(|| host.clone()),
                    host_name: host.clone(),
                    version: Some(response.version.name),
                    online_players: players.online,
                    max_players: players.max,
                    ping_ms: Some(start.elapsed().as_millis() as u64),
                });
            }
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("failed to query external server status")))
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

pub async fn detect_minecraft_nickname() -> Result<MinecraftNicknameDetection> {
    task::spawn_blocking(detect_minecraft_nickname_blocking)
        .await
        .context("failed to await Minecraft nickname detector task")?
}

async fn query_status(host: &str, port: u16, protocol_version: i32) -> Result<StatusResponse> {
    let target = format!("{host}:{port}");
    let mut stream = timeout(Duration::from_secs(2), TcpStream::connect(&target))
        .await
        .context("timed out while connecting to the local Minecraft target")?
        .with_context(|| format!("failed to connect to {target}"))?;

    let handshake = build_handshake_packet(host, port, protocol_version)?;
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
    Ok(response)
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
        roots.push(app_data.join(".minecraft"));
        roots.push(app_data.join("PrismLauncher").join("instances"));
        roots.push(app_data.join("MultiMC").join("instances"));
        roots.push(app_data.join(".feather").join("logs"));
    }

    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let user_profile = PathBuf::from(user_profile);
        roots.push(
            user_profile
                .join("curseforge")
                .join("minecraft")
                .join("Instances"),
        );
        roots.push(user_profile.join("AppData").join("Roaming").join(".minecraft").join("logs"));
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
    if depth > 6 || !root.exists() {
        return;
    }

    if root.is_file() {
        if root
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.eq_ignore_ascii_case("log"))
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
    for marker in [
        "Started serving on ",
        "Started serving on port ",
        "Local game hosted on port ",
        "Local server started on port ",
        "Порт локального сервера: ",
        "Local server port: ",
    ] {
        if let Some(index) = line.find(marker) {
            let port_part = &line[index + marker.len()..];
            let digits = port_part
                .chars()
                .take_while(|value| value.is_ascii_digit())
                .collect::<String>();
            if let Ok(port) = digits.parse() {
                return Some(port);
            }
        }
    }
    None
}

fn detect_minecraft_nickname_blocking() -> Result<MinecraftNicknameDetection> {
    let candidates = collect_nickname_sources();
    for path in candidates {
        if !path.exists() {
            continue;
        }
        if let Ok(contents) = fs::read_to_string(&path) {
            if let Some(nickname) = parse_nickname_from_file(&path, &contents) {
                return Ok(MinecraftNicknameDetection {
                    nickname,
                    source_path: path.display().to_string(),
                });
            }
        }
    }
    Err(anyhow!("could not detect minecraft nickname from launcher files or logs"))
}

fn collect_nickname_sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(app_data) = std::env::var_os("APPDATA") {
        let app_data = PathBuf::from(app_data);
        files.push(app_data.join(".minecraft").join("launcher_accounts.json"));
        files.push(app_data.join(".minecraft").join("launcher_profiles.json"));
        files.push(app_data.join(".minecraft").join("logs").join("latest.log"));
    }
    if let Some(user_profile) = std::env::var_os("USERPROFILE") {
        let user_profile = PathBuf::from(user_profile);
        files.push(
            user_profile
                .join("AppData")
                .join("Roaming")
                .join(".minecraft")
                .join("logs")
                .join("latest.log"),
        );
    }
    files
}

fn parse_nickname_from_file(path: &Path, contents: &str) -> Option<String> {
    let name = path.file_name()?.to_string_lossy().to_lowercase();
    if name == "launcher_accounts.json" {
        return parse_launcher_accounts_nick(contents);
    }
    if name == "launcher_profiles.json" {
        return parse_launcher_profiles_nick(contents);
    }
    parse_logs_nick(contents)
}

fn parse_launcher_accounts_nick(contents: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(contents).ok()?;
    let active_id = value.get("activeAccountLocalId")?.as_str()?;
    value.get("accounts")?
        .get(active_id)?
        .get("minecraftProfile")?
        .get("name")?
        .as_str()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
}

fn parse_launcher_profiles_nick(contents: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(contents).ok()?;
    let selected_profile = value
        .get("selectedUser")
        .and_then(|item| item.get("profile"))
        .and_then(|item| item.as_str())?;
    let auth_db = value.get("authenticationDatabase")?.as_object()?;
    for account in auth_db.values() {
        if let Some(display_name) = account
            .get("profiles")
            .and_then(|profiles| profiles.get(selected_profile))
            .and_then(|profile| profile.get("displayName"))
            .and_then(|name| name.as_str())
            .map(|name| name.trim().to_string())
            .filter(|name| !name.is_empty())
        {
            return Some(display_name);
        }
    }
    None
}

fn parse_logs_nick(contents: &str) -> Option<String> {
    for line in contents.lines().rev() {
        if let Some(index) = line.find("Setting user: ") {
            let part = &line[index + "Setting user: ".len()..];
            let nick = part
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .collect::<String>();
            if !nick.is_empty() {
                return Some(nick);
            }
        }
    }
    None
}

fn status_description_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.trim().to_string()),
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(|value| value.as_str()) {
                if !text.trim().is_empty() {
                    return Some(text.trim().to_string());
                }
            }
            if let Some(extra) = map.get("extra").and_then(|value| value.as_array()) {
                let combined = extra
                    .iter()
                    .filter_map(status_description_to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
                    .trim()
                    .to_string();
                if !combined.is_empty() {
                    return Some(combined);
                }
            }
            None
        }
        _ => None,
    }
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
