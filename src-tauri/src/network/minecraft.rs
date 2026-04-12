use anyhow::{anyhow, Context, Result};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use serde::Deserialize;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    task,
    time::{timeout, Duration},
};

use crate::models::{
    ExternalServerProbe, LanPortDetection, LocalPlayerSnapshot, LocalTargetState,
    MinecraftClientRuntimeInfo, MinecraftNicknameDetection, PreflightReport,
};

const STATUS_PROTOCOL_CANDIDATES: &[i32] = &[767, 764, 760, 47];
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
    #[serde(default)]
    sample: Vec<MinecraftPlayerSample>,
}

#[derive(Debug, Deserialize)]
struct MinecraftPlayerSample {
    name: String,
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
                let players = response.players.unwrap_or(MinecraftPlayers {
                    online: 0,
                    max: 0,
                    sample: Vec::new(),
                });
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

pub async fn detect_client_runtime_info() -> Result<MinecraftClientRuntimeInfo> {
    task::spawn_blocking(detect_client_runtime_info_blocking)
        .await
        .context("failed to await Minecraft runtime detector task")?
}

pub async fn read_local_player_snapshot(port: u16) -> Result<LocalPlayerSnapshot> {
    let response = detect_status_response("127.0.0.1", port).await;
    match response {
        Ok(res) => {
            let players = res.players.unwrap_or(MinecraftPlayers {
                online: 0,
                max: 0,
                sample: Vec::new(),
            });
            Ok(LocalPlayerSnapshot {
                online_players: players.online,
                max_players: players.max,
                sample_names: players
                    .sample
                    .into_iter()
                    .filter_map(|sample| sanitize_minecraft_nickname(&sample.name))
                    .collect(),
            })
        }
        Err(error) => {
            tracing::debug!("Failed to read local player snapshot: {error:#}");
            Ok(LocalPlayerSnapshot {
                online_players: 0,
                max_players: 0,
                sample_names: Vec::new(),
            })
        }
    }
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

async fn detect_status_response(host: &str, port: u16) -> Result<StatusResponse> {
    let mut last_error = None;
    for protocol_version in STATUS_PROTOCOL_CANDIDATES {
        match query_status(host, port, *protocol_version).await {
            Ok(response) => return Ok(response),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow!("failed to get a valid Minecraft status response")))
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
        if let Some(contents) = read_text_lossy(&path) {
            if let Some(detection) = parse_lan_port_from_contents(&path, &contents) {
                return Ok(detection);
            }
        }
    }

    if let Some(detection) = detect_lan_port_from_system_listeners() {
        return Ok(detection);
    }

    Err(anyhow!(
        "could not find a recent 'Started serving on <port>' entry in Minecraft logs or active Java listeners"
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
        roots.push(app_data.join(".tlauncher").join("legacy").join("Minecraft").join("logs"));
        roots.push(
            app_data
                .join(".tlauncher")
                .join("legacy")
                .join("Minecraft")
                .join("game")
                .join("logs"),
        );
    }

    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        roots.push(local_app_data.join(".minecraft").join("logs"));
        roots.push(local_app_data.join("PrismLauncher").join("instances"));
        roots.push(local_app_data.join("MultiMC").join("instances"));
        roots.push(local_app_data.join("curseforge").join("minecraft").join("Instances"));
        roots.push(local_app_data.join("curseforge").join("minecraft").join("Install"));
        roots.push(
            local_app_data
                .join("Packages")
                .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                .join("LocalCache")
                .join("Roaming")
                .join(".minecraft")
                .join("logs"),
        );
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
        let extension = root.extension().and_then(|value| value.to_str()).unwrap_or_default();
        if extension.eq_ignore_ascii_case("log") || extension.eq_ignore_ascii_case("txt")
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
                            .map(|value| {
                                value.eq_ignore_ascii_case("log") || value.eq_ignore_ascii_case("txt")
                            })
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
    let normalized_line = line.trim();
    for marker in [
        "Started serving on ",
        "Started serving on port ",
        "Local game hosted on port ",
        "Local server started on port ",
        "Порт локального сервера: ",
        "Local server port: ",
    ] {
        if let Some(index) = normalized_line.find(marker) {
            let port_part = &normalized_line[index + marker.len()..];
            let digits = port_part
                .chars()
                .take_while(|value| value.is_ascii_digit())
                .collect::<String>();
            if let Ok(port) = digits.parse() {
                return Some(port);
            }
        }
    }

    let lower = normalized_line.to_ascii_lowercase();
    if lower.contains("started serving on")
        || lower.contains("local server port")
        || lower.contains("порт локального сервера")
    {
        return extract_last_port_from_line(normalized_line);
    }

    None
}

fn extract_last_port_from_line(line: &str) -> Option<u16> {
    let mut current = String::new();
    let mut last_valid = None;

    for ch in line.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if let Some(port) = parse_port_candidate(&current) {
            last_valid = Some(port);
        }
        current.clear();
    }

    if let Some(port) = parse_port_candidate(&current) {
        last_valid = Some(port);
    }

    last_valid
}

fn parse_port_candidate(value: &str) -> Option<u16> {
    if value.len() < 2 || value.len() > 5 {
        return None;
    }
    let port = value.parse::<u16>().ok()?;
    (port > 0).then_some(port)
}

fn detect_lan_port_from_system_listeners() -> Option<LanPortDetection> {
    let output = hidden_command("netstat")
        .args(["-ano", "-p", "tcp"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let content = String::from_utf8_lossy(&output.stdout);
    let mut java_candidates = Vec::new();
    let mut generic_candidates = Vec::new();
    let mut java_pid_cache: HashMap<u32, bool> = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("TCP") {
            continue;
        }
        let columns = trimmed.split_whitespace().collect::<Vec<_>>();
        if columns.len() < 5 {
            continue;
        }
        if !columns[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }

        let local = columns[1];
        let Ok(pid) = columns[4].parse::<u32>() else {
            continue;
        };
        let Some((host, port)) = split_host_port_label(local) else {
            continue;
        };
        if port == 0 || port < 1024 || !is_local_bind_host(&host) {
            continue;
        }

        let candidate = (port, pid, local.to_string());
        if *java_pid_cache
            .entry(pid)
            .or_insert_with(|| pid_looks_like_java(pid))
        {
            java_candidates.push(candidate);
        } else {
            generic_candidates.push(candidate);
        }
    }

    java_candidates.sort_by(|left, right| right.0.cmp(&left.0));
    generic_candidates.sort_by(|left, right| right.0.cmp(&left.0));

    let picked = java_candidates
        .first()
        .or_else(|| generic_candidates.first())?;

    Some(LanPortDetection {
        port: picked.0,
        source_path: "system:netstat".into(),
        source_line: format!("netstat LISTENING {} pid {}", picked.2, picked.1),
    })
}

fn split_host_port_label(value: &str) -> Option<(String, u16)> {
    if value.is_empty() {
        return None;
    }

    if value.starts_with('[') {
        let close = value.find(']')?;
        let host = value.get(1..close)?.to_string();
        let port = value.get(close + 2..)?.parse::<u16>().ok()?;
        return Some((host, port));
    }

    let (host, port) = value.rsplit_once(':')?;
    Some((host.to_string(), port.parse::<u16>().ok()?))
}

fn is_local_bind_host(host: &str) -> bool {
    let normalized = host.trim().trim_matches(|c| c == '[' || c == ']').to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "127.0.0.1" | "0.0.0.0" | "::1" | "::" | "*" | "localhost"
    )
}

fn pid_looks_like_java(pid: u32) -> bool {
    let output = hidden_command("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output();

    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("INFO:"));
    let Some(line) = line else {
        return false;
    };
    let first = line.split(',').next().unwrap_or_default();
    let process_name = first.trim().trim_matches('"').to_ascii_lowercase();
    process_name.contains("java")
}

fn hidden_command(program: &str) -> Command {
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

fn detect_minecraft_nickname_blocking() -> Result<MinecraftNicknameDetection> {
    let candidates = collect_nickname_sources();
    for path in candidates {
        if !path.exists() {
            continue;
        }
        if let Some(contents) = read_text_lossy(&path) {
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

fn detect_client_runtime_info_blocking() -> Result<MinecraftClientRuntimeInfo> {
    let nickname = detect_minecraft_nickname_blocking().ok();
    let candidates = collect_nickname_sources();

    for path in candidates {
        if !path.exists() {
            continue;
        }
        let Some(contents) = read_text_lossy(&path) else {
            continue;
        };
        let launcher = infer_launcher_from_path(&path);
        let (minecraft_version, mod_loader) = infer_runtime_from_file(&path, &contents);
        if launcher.is_some() || minecraft_version.is_some() || mod_loader.is_some() {
            return Ok(MinecraftClientRuntimeInfo {
                nickname: nickname.as_ref().map(|value| value.nickname.clone()),
                launcher,
                minecraft_version,
                mod_loader,
                source_path: Some(path.display().to_string()),
                note: nickname.as_ref().map(|value| format!("nickname source: {}", value.source_path)),
            });
        }
    }

    Err(anyhow!(
        "could not detect launcher, Minecraft version, or mod loader from local launcher files/logs"
    ))
}

fn collect_nickname_sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Some(app_data) = std::env::var_os("APPDATA") {
        let app_data = PathBuf::from(app_data);
        files.push(app_data.join(".minecraft").join("launcher_accounts.json"));
        files.push(app_data.join(".minecraft").join("launcher_profiles.json"));
        files.push(app_data.join(".minecraft").join("logs").join("latest.log"));
        files.push(
            app_data
                .join(".tlauncher")
                .join("legacy")
                .join("Minecraft")
                .join("logs")
                .join("latest.log"),
        );
        files.push(
            app_data
                .join(".tlauncher")
                .join("legacy")
                .join("Minecraft")
                .join("game")
                .join("logs")
                .join("latest.log"),
        );
    }
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        let local_app_data = PathBuf::from(local_app_data);
        files.push(local_app_data.join(".minecraft").join("launcher_accounts.json"));
        files.push(local_app_data.join(".minecraft").join("launcher_profiles.json"));
        files.push(local_app_data.join(".minecraft").join("logs").join("latest.log"));
        files.push(
            local_app_data
                .join("Packages")
                .join("Microsoft.4297127D64EC6_8wekyb3d8bbwe")
                .join("LocalCache")
                .join("Roaming")
                .join(".minecraft")
                .join("logs")
                .join("latest.log"),
        );
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
    if let Ok(candidates) = collect_log_candidates() {
        files.extend(candidates.into_iter().take(64));
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
    if name.ends_with(".log") || name.ends_with(".txt") {
        return parse_logs_nick(contents);
    }
    None
}

fn parse_launcher_accounts_nick(contents: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(contents).ok()?;
    let accounts = value.get("accounts")?.as_object()?;

    let active_id = value.get("activeAccountLocalId").and_then(|item| {
        item.as_str()
            .map(str::to_string)
            .or_else(|| item.as_u64().map(|id| id.to_string()))
    });

    if let Some(active_id) = active_id {
        if let Some(name) = accounts
            .get(&active_id)
            .and_then(|account| account.get("minecraftProfile"))
            .and_then(|profile| profile.get("name"))
            .and_then(|item| item.as_str())
            .and_then(sanitize_minecraft_nickname)
        {
            return Some(name);
        }
    }

    for account in accounts.values() {
        if let Some(name) = account
            .get("minecraftProfile")
            .and_then(|profile| profile.get("name"))
            .and_then(|item| item.as_str())
            .and_then(sanitize_minecraft_nickname)
        {
            return Some(name);
        }
    }

    None
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

    value
        .as_object()
        .and_then(|obj| obj.get("profiles"))
        .and_then(|profiles| profiles.as_object())
        .and_then(|profiles| profiles.values().find_map(|profile| profile.get("name")))
        .and_then(|name| name.as_str())
        .and_then(sanitize_minecraft_nickname)
}

fn parse_logs_nick(contents: &str) -> Option<String> {
    for line in contents.lines().rev() {
        for marker in ["Setting user: ", "Session Name is ", "Username: ", "Logged in as "] {
            if let Some(index) = line.find(marker) {
                let part = &line[index + marker.len()..];
                let nick = part
                    .chars()
                    .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                    .collect::<String>();
                if let Some(name) = sanitize_minecraft_nickname(&nick) {
                    return Some(name);
                }
            }
        }
    }
    None
}

fn infer_runtime_from_file(path: &Path, contents: &str) -> (Option<String>, Option<String>) {
    let name = path.file_name().and_then(|value| value.to_str()).unwrap_or_default().to_ascii_lowercase();
    if name == "launcher_profiles.json" || name == "launcher_accounts.json" {
        let version = parse_launcher_version(contents);
        return (version, None);
    }
    if name.ends_with(".log") || name.ends_with(".txt") {
        return parse_log_runtime(contents);
    }
    (None, None)
}

fn parse_launcher_version(contents: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(contents).ok()?;
    value
        .as_object()
        .and_then(|obj| obj.get("profiles"))
        .and_then(|profiles| profiles.as_object())
        .and_then(|profiles| {
            profiles.values().find_map(|profile| {
                profile
                    .get("lastVersionId")
                    .and_then(|item| item.as_str())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
}

fn parse_log_runtime(contents: &str) -> (Option<String>, Option<String>) {
    let mut minecraft_version = None;
    let mut mod_loader = None;

    for line in contents.lines().rev() {
        let trimmed = line.trim();
        if minecraft_version.is_none() {
            if let Some((version, loader)) = parse_fabric_runtime(trimmed) {
                minecraft_version = Some(version);
                mod_loader = Some(loader);
                break;
            }
            if let Some((version, loader)) = parse_quilt_runtime(trimmed) {
                minecraft_version = Some(version);
                mod_loader = Some(loader);
                break;
            }
            if let Some((version, loader)) = parse_forge_runtime(trimmed) {
                minecraft_version = Some(version);
                mod_loader = Some(loader);
                break;
            }
            if let Some(version) = parse_launched_version(trimmed) {
                minecraft_version = Some(version);
            }
        }
    }

    (minecraft_version, mod_loader)
}

fn parse_fabric_runtime(line: &str) -> Option<(String, String)> {
    let (_, tail) = line.split_once("Loading Minecraft ")?;
    let (version, loader_tail) = tail.split_once(" with Fabric Loader ")?;
    Some((version.trim().to_string(), format!("Fabric {}", loader_tail.trim())))
}

fn parse_quilt_runtime(line: &str) -> Option<(String, String)> {
    let (_, tail) = line.split_once("Loading Minecraft ")?;
    let (version, loader_tail) = tail.split_once(" with Quilt Loader ")?;
    Some((version.trim().to_string(), format!("Quilt {}", loader_tail.trim())))
}

fn parse_forge_runtime(line: &str) -> Option<(String, String)> {
    let (_, tail) = line.split_once("Forge mod loading, version ")?;
    let (forge_version, rest) = tail.split_once(", for MC ")?;
    let mc_version = rest
        .split_whitespace()
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    Some((mc_version.to_string(), format!("Forge {}", forge_version.trim())))
}

fn parse_launched_version(line: &str) -> Option<String> {
    let (_, tail) = line.split_once("Launched Version: ")?;
    let version = tail.split_whitespace().next()?.trim();
    (!version.is_empty()).then(|| version.to_string())
}

fn infer_launcher_from_path(path: &Path) -> Option<String> {
    let normalized = path.display().to_string().to_ascii_lowercase();
    if normalized.contains("prismlauncher") {
        return Some("PrismLauncher".into());
    }
    if normalized.contains("multimc") {
        return Some("MultiMC".into());
    }
    if normalized.contains("curseforge") {
        return Some("CurseForge".into());
    }
    if normalized.contains(".tlauncher") {
        return Some("TLauncher".into());
    }
    if normalized.contains("microsoft.4297127d64ec6_8wekyb3d8bbwe") {
        return Some("Minecraft Launcher (MS Store)".into());
    }
    if normalized.contains(".minecraft") {
        return Some("Minecraft Launcher".into());
    }
    None
}

fn sanitize_minecraft_nickname(value: &str) -> Option<String> {
    let normalized = value.trim();
    if normalized.len() < 3 || normalized.len() > 16 {
        return None;
    }
    normalized
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        .then_some(normalized.to_string())
}

fn read_text_lossy(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    Some(String::from_utf8_lossy(&bytes).to_string())
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
