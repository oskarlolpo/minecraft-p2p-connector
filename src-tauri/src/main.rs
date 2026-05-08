#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod cert;
mod models;
mod network;
mod signaling;

use network::minecraft::{
    build_preflight_report, detect_client_runtime_info, detect_lan_port_from_logs,
    detect_minecraft_nickname, get_available_lan_ports_command, probe_external_server,
    read_local_player_snapshot,
};
use network::manager::NetworkManager;
use network::geyser::GeyserManager;
use network::stun;
use network::test_server::{probe_test_server, TestServerManager};
use models::{
    AppInfo, DiagnosticSnapshot, ExternalServerProbe, InstallUpdateResult, LanPortDetection,
    LocalPlayerSnapshot, MinecraftClientRuntimeInfo, MinecraftNicknameDetection, NetworkStatus,
    PreflightReport, SwarmBootstrap, TestServerInfo, UpdateCheckResult,
};
use network::stun::NatTypeResult;
use std::{path::PathBuf, process::Command, time::{SystemTime, UNIX_EPOCH}};
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

const RELEASES_API_URL: &str =
    "https://api.github.com/repos/oskarlolpo/minecraft-p2p-connector/releases/latest";
const RELEASES_LATEST_URL: &str =
    "https://github.com/oskarlolpo/minecraft-p2p-connector/releases/latest";

#[derive(Clone)]
struct AppState {
    manager: NetworkManager,
    geyser: GeyserManager,
    test_server: TestServerManager,
    last_preflight: std::sync::Arc<Mutex<Option<PreflightReport>>>,
}

#[tauri::command]
async fn start_hosting(
    app: AppHandle,
    state: State<'_, AppState>,
    room_name: String,
    password: Option<String>,
    local_port: u16,
    enable_geyser: bool,
    geyser_port: Option<u16>,
    enable_e4mc: Option<bool>,
) -> Result<SwarmBootstrap, String> {
    // Preflight: check that the local game port is actually reachable
    if let Err(e) = stun::preflight_port_check(local_port) {
        tracing::warn!("Preflight port check failed: {e:#}");
        // Non-fatal: log but proceed (Minecraft may be using the port)
    }

    let room_name_for_geyser = room_name.clone();
    let public_addr = state
        .manager
        .start_hosting(
            app,
            room_name,
            password,
            local_port,
            enable_e4mc.unwrap_or(state.manager.e4mc_enabled_by_default()),
        )
        .await
        .map_err(|error| format!("{error:#}"))?;

    let bedrock_public_endpoint =
        derive_bedrock_public_endpoint(&public_addr, geyser_port.unwrap_or(19132));

    let geyser_info = if enable_geyser {
        match state
            .geyser
            .start(
                local_port,
                &room_name_for_geyser,
                geyser_port,
                bedrock_public_endpoint.clone(),
            )
            .await
        {
            Ok(info) => Some(info),
            Err(error) => {
                let _ = state.manager.stop_hosting().await;
                return Err(format!("{error:#}"));
            }
        }
    } else {
        let _ = state.geyser.stop().await;
        None
    };

    if let Some(info) = geyser_info {
        let shared = state.manager.shared_status();
        let mut status = shared.write().await;
        status.geyser_enabled = true;
        status.bedrock_port = info.bedrock_port;
        status.note = Some(format!(
            "Host is active. Java players use the normal room flow, Bedrock players connect to {}.",
            info
                .bedrock_public_endpoint
                .clone()
                .unwrap_or_else(|| format!("UDP {}", info.bedrock_port.unwrap_or(19132)))
        ));
        status.logs.insert(
            0,
            format!(
                "Geyser bridge ready: {} -> Java 127.0.0.1:{local_port}",
                info
                    .bedrock_public_endpoint
                    .clone()
                    .unwrap_or_else(|| format!("UDP {}", info.bedrock_port.unwrap_or(19132)))
            ),
        );
        if let Some(rule_name) = &info.firewall_rule_name {
            status
                .logs
                .insert(1, format!("Windows Firewall rule ready: {rule_name}"));
        }
    }

    Ok(SwarmBootstrap {
        peer_id: String::new(),
        listen_addrs: vec![normalize_socket_addr_to_multiaddr(&public_addr)],
        relay_addrs: Vec::new(),
        nat_status: "quic-direct".into(),
        local_game_port: Some(local_port),
    })
}

#[tauri::command]
async fn stop_hosting(state: State<'_, AppState>) -> Result<(), String> {
    let _ = state.geyser.stop().await;
    state
        .manager
        .stop_hosting()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn connect_to_peer(
    app: AppHandle,
    state: State<'_, AppState>,
    peer_id: String,
    peer_addrs: Vec<String>,
    relay_session_id: Option<String>,
) -> Result<(), String> {
    let peer_addr = peer_addrs
        .iter()
        .find_map(|value| multiaddr_or_socket_to_socket(value))
        .ok_or_else(|| "не удалось извлечь socket address из peerAddrs".to_string())?;

    state
        .manager
        .connect_to_peer(
            app,
            peer_addr,
            (!peer_id.trim().is_empty()).then_some(peer_id),
            relay_session_id,
        )
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn prepare_client_connect(
    state: State<'_, AppState>,
    peer_id: String,
    peer_addrs: Vec<String>,
) -> Result<(), String> {
    let peer_addr = peer_addrs
        .iter()
        .find_map(|value| multiaddr_or_socket_to_socket(value))
        .ok_or_else(|| "не удалось извлечь socket address из peerAddrs".to_string())?;

    state
        .manager
        .prepare_client_connect(peer_addr, (!peer_id.trim().is_empty()).then_some(peer_id))
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn commit_prepared_client_connect(
    app: AppHandle,
    state: State<'_, AppState>,
    relay_session_id: Option<String>,
) -> Result<(), String> {
    state
        .manager
        .commit_prepared_client_connect(app, relay_session_id)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn kick_peer(state: State<'_, AppState>, peer_id: String) -> Result<(), String> {
    state
        .manager
        .kick_peer(peer_id)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn get_status(state: State<'_, AppState>) -> Result<NetworkStatus, String> {
    Ok(state.manager.get_status().await)
}

#[tauri::command]
async fn run_preflight(local_port: u16) -> Result<models::PreflightReport, String> {
    Ok(build_preflight_report(local_port).await)
}

#[tauri::command]
async fn detect_lan_port() -> Result<LanPortDetection, String> {
    detect_lan_port_from_logs()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn detect_minecraft_nickname_command() -> Result<MinecraftNicknameDetection, String> {
    detect_minecraft_nickname()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn detect_client_runtime_info_command() -> Result<MinecraftClientRuntimeInfo, String> {
    detect_client_runtime_info()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn get_local_player_snapshot_command(port: u16) -> Result<LocalPlayerSnapshot, String> {
    read_local_player_snapshot(port)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn query_external_server(host: String, port: u16) -> Result<ExternalServerProbe, String> {
    probe_external_server(host, port)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn get_app_info() -> Result<AppInfo, String> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        product_name: "Minecraft P2P Connector".into(),
    })
}

#[tauri::command]
async fn check_for_updates() -> Result<UpdateCheckResult, String> {
    check_for_updates_impl()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn install_update() -> Result<InstallUpdateResult, String> {
    install_update_impl()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn run_preflight_and_store(
    state: State<'_, AppState>,
    local_port: u16,
) -> Result<PreflightReport, String> {
    let report = build_preflight_report(local_port).await;
    *state.last_preflight.lock().await = Some(report.clone());
    Ok(report)
}

#[tauri::command]
async fn start_test_server(
    app: AppHandle,
    state: State<'_, AppState>,
    port: u16,
) -> Result<TestServerInfo, String> {
    state
        .test_server
        .start(app, state.manager.shared_status(), port)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn stop_test_server(state: State<'_, AppState>) -> Result<(), String> {
    state
        .test_server
        .stop(state.manager.shared_status())
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn probe_test_server_command(port: u16, payload: Option<String>) -> Result<String, String> {
    probe_test_server(
        port,
        payload.unwrap_or_else(|| "diagnostic-ping".into()),
    )
    .await
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn detect_nat_type_command() -> Result<NatTypeResult, String> {
    Ok(stun::detect_nat_type().await)
}

#[tauri::command]
async fn preflight_port_check_command(port: u16) -> Result<(), String> {
    stun::preflight_port_check(port).map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn export_diagnostics_snapshot(
    state: State<'_, AppState>,
    local_port: Option<u16>,
) -> Result<DiagnosticSnapshot, String> {
    let status = state.manager.get_status().await;
    let preflight = match local_port.or(status.local_game_port) {
        Some(port) => Some(build_preflight_report(port).await),
        None => state.last_preflight.lock().await.clone(),
    };
    let test_server = state.test_server.current_info().await;
    let geyser = state.geyser.current_info().await;
    let exported_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".into());

    Ok(DiagnosticSnapshot {
        exported_at,
        role: status.mode,
        status,
        preflight,
        test_server,
        geyser,
    })
}

async fn check_for_updates_impl() -> anyhow::Result<UpdateCheckResult> {
    let client = reqwest::Client::builder()
        .user_agent("minecraft-p2p-connector")
        .build()?;

    if let Ok(response) = client
        .get(RELEASES_API_URL)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
    {
        if response.status().is_success() {
            let value: serde_json::Value = response.json().await?;
            let tag_name = value
                .get("tag_name")
                .and_then(|item| item.as_str())
                .unwrap_or_default()
                .trim()
                .to_string();
            let latest_version = tag_name.trim_start_matches('v').to_string();
            let current_version = env!("CARGO_PKG_VERSION").to_string();
            let release_url = value
                .get("html_url")
                .and_then(|item| item.as_str())
                .map(str::to_string);
            let download_url = value
                .get("assets")
                .and_then(|item| item.as_array())
                .and_then(|assets| {
                    assets.iter().find_map(|asset| {
                        let name = asset.get("name")?.as_str()?;
                        if name.ends_with("_x64-setup.exe") {
                            asset
                                .get("browser_download_url")
                                .and_then(|item| item.as_str())
                                .map(str::to_string)
                        } else {
                            None
                        }
                    })
                })
                .or_else(|| {
                    (!tag_name.is_empty())
                        .then(|| build_setup_asset_url(&tag_name, &latest_version))
                });

            return Ok(UpdateCheckResult {
                current_version: current_version.clone(),
                latest_version: (!latest_version.is_empty()).then_some(latest_version.clone()),
                available: !latest_version.is_empty() && latest_version != current_version,
                release_url,
                download_url,
            });
        }
    }

    let latest_response = client.get(RELEASES_LATEST_URL).send().await?.error_for_status()?;
    let resolved_url = latest_response.url().to_string();
    let tag_name = extract_tag_from_release_url(&resolved_url)
        .ok_or_else(|| anyhow::anyhow!("failed to resolve latest GitHub release tag"))?;
    let latest_version = tag_name.trim_start_matches('v').to_string();
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    let release_url = Some(format!(
        "https://github.com/oskarlolpo/minecraft-p2p-connector/releases/tag/{tag_name}"
    ));
    let download_url = Some(build_setup_asset_url(&tag_name, &latest_version));

    Ok(UpdateCheckResult {
        current_version: current_version.clone(),
        latest_version: (!latest_version.is_empty()).then_some(latest_version.clone()),
        available: !latest_version.is_empty() && latest_version != current_version,
        release_url,
        download_url,
    })
}

async fn install_update_impl() -> anyhow::Result<InstallUpdateResult> {
    let update = check_for_updates_impl().await?;
    if !update.available {
        return Ok(InstallUpdateResult {
            message: "Обновлений нет.".into(),
        });
    }

    let download_url = update
        .download_url
        .clone()
        .ok_or_else(|| anyhow::anyhow!("release asset not found"))?;
    let temp_path = std::env::temp_dir().join(format!(
        "Minecraft.P2P.Connector_{}_setup.exe",
        update.latest_version.clone().unwrap_or_else(|| "latest".into())
    ));

    let client = reqwest::Client::builder()
        .user_agent("minecraft-p2p-connector")
        .build()?;
    let bytes = client
        .get(download_url.clone())
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    tokio::fs::write(&temp_path, &bytes).await?;

    launch_file_detached(&temp_path)?;

    Ok(InstallUpdateResult {
        message: format!("Установщик загружен и запущен: {}", temp_path.display()),
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "minecraft_p2p_connector=info,quinn=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState {
            manager: NetworkManager::new(),
            geyser: GeyserManager::new(),
            test_server: TestServerManager::new(),
            last_preflight: std::sync::Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            start_hosting,
            stop_hosting,
            connect_to_peer,
            prepare_client_connect,
            commit_prepared_client_connect,
            kick_peer,
            get_status,
            run_preflight,
            detect_lan_port,
            detect_minecraft_nickname_command,
            detect_client_runtime_info_command,
            get_local_player_snapshot_command,
            query_external_server,
            get_app_info,
            check_for_updates,
            install_update,
            run_preflight_and_store,
            start_test_server,
            stop_test_server,
            probe_test_server_command,
            export_diagnostics_snapshot,
            get_available_lan_ports_command,
            detect_nat_type_command,
            preflight_port_check_command
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn normalize_socket_addr_to_multiaddr(value: &str) -> String {
    if let Ok(socket) = value.parse::<std::net::SocketAddr>() {
        match socket.ip() {
            std::net::IpAddr::V4(ip) => format!("/ip4/{ip}/udp/{}/quic-v1", socket.port()),
            std::net::IpAddr::V6(ip) => format!("/ip6/{ip}/udp/{}/quic-v1", socket.port()),
        }
    } else {
        value.to_string()
    }
}

fn multiaddr_or_socket_to_socket(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.contains(':') && !trimmed.starts_with('/') {
        return Some(trimmed.to_string());
    }

    let parts = trimmed.split('/').collect::<Vec<_>>();
    if parts.len() >= 5 && parts[1] == "ip4" && (parts[3] == "tcp" || parts[3] == "udp") {
        return Some(format!("{}:{}", parts[2], parts[4]));
    }
    if parts.len() >= 5 && parts[1] == "ip6" && (parts[3] == "tcp" || parts[3] == "udp") {
        return Some(format!("[{}]:{}", parts[2], parts[4]));
    }

    None
}

fn extract_tag_from_release_url(url: &str) -> Option<String> {
    let (_, tail) = url.split_once("/tag/")?;
    let tag = tail
        .split(['?', '#', '/'])
        .next()
        .map(str::trim)
        .unwrap_or_default();
    (!tag.is_empty()).then(|| tag.to_string())
}

fn build_setup_asset_url(tag_name: &str, latest_version: &str) -> String {
    format!(
        "https://github.com/oskarlolpo/minecraft-p2p-connector/releases/download/{tag_name}/Minecraft.P2P.Connector_{}_x64-setup.exe",
        latest_version
    )
}

fn derive_bedrock_public_endpoint(public_addr: &str, bedrock_port: u16) -> Option<String> {
    let trimmed = public_addr.trim();
    let socket = trimmed.parse::<std::net::SocketAddr>().ok()?;
    Some(match socket.ip() {
        std::net::IpAddr::V4(ip) => format!("{ip}:{bedrock_port}"),
        std::net::IpAddr::V6(ip) => format!("[{ip}]:{bedrock_port}"),
    })
}

fn launch_file_detached(path: &PathBuf) -> anyhow::Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .args(["/C", "start", "", &path.display().to_string()])
            .creation_flags(0x0800_0000)
            .spawn()?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(anyhow::anyhow!("update installation is only supported on Windows"))
}

