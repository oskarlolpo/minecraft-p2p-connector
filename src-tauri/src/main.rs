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
use tauri::{AppHandle, State, Emitter};
use tokio::sync::Mutex;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
    minecraft_version: Option<String>,
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
            app.clone(),
            room_name,
            password,
            local_port,
            enable_e4mc.unwrap_or(state.manager.e4mc_enabled_by_default()),
            minecraft_version,
        )
        .await
        .map_err(|error| format!("{error:#}"))?;

    let bedrock_public_endpoint =
        derive_bedrock_public_endpoint(&public_addr, geyser_port.unwrap_or(19132));

    let geyser_info = if enable_geyser {
        match state
            .geyser
            .start(
                app.clone(),
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
    room_name: Option<String>,
    host_name: Option<String>,
    mc_version: Option<String>,
    slots: Option<String>,
) -> Result<(), String> {
    let peer_addr = peer_addrs
        .iter()
        .find_map(|value| multiaddr_or_socket_to_socket(value))
        .ok_or_else(|| "не удалось извлечь socket address из peerAddrs".to_string())?;

    state
        .manager
        .prepare_client_connect(
            peer_addr,
            (!peer_id.trim().is_empty()).then_some(peer_id),
            room_name,
            host_name,
            mc_version,
            slots,
        )
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn commit_prepared_client_connect(
    app: AppHandle,
    state: State<'_, AppState>,
    relay_session_id: Option<String>,
    use_udp: Option<bool>,
) -> Result<(), String> {
    state
        .manager
        .commit_prepared_client_connect(app, relay_session_id, use_udp.unwrap_or(false))
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
async fn get_latest_bedrock_world_name_command() -> Result<String, String> {
    network::minecraft::get_latest_bedrock_world_name()
        .ok_or_else(|| "Could not find latest Bedrock world name".to_string())
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
            get_latest_bedrock_world_name_command,
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
            preflight_port_check_command,
            open_url,
            start_oauth_server
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

#[derive(serde::Serialize, Clone)]
struct OAuthPayload {
    access_token: Option<String>,
    refresh_token: Option<String>,
    code: Option<String>,
}

#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|e| format!("Failed to open URL: {e}"))?;
        return Ok(());
    }
    
    #[allow(unreachable_code)]
    Err("Opening URL is only supported on Windows".into())
}

#[tauri::command]
async fn start_oauth_server(app: AppHandle) -> Result<u16, String> {
    let port: u16 = 14235;
    // Try binding; if port busy (e.g. previous session still running), just return ok
    let listener = match tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await {
        Ok(l) => l,
        Err(_) => {
            println!("[auth-server] Port {port} already in use вЂ” assuming server already running");
            return Ok(port);
        }
    };
    
    let app_clone = app.clone();
    tokio::spawn(async move {
        let mut access_token = String::new();
        let mut refresh_token = String::new();
        let mut code = String::new();
        
        let server_future = async {
            loop {
                let (mut stream, _) = match listener.accept().await {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                
                let mut request_bytes = Vec::new();
                let mut temp_buf = [0u8; 4096];
                let mut headers_length = 0usize;
                let mut content_length = 0usize;
                let mut is_post = false;
                
                println!("[auth-server] New TCP client connected. Reading data...");
                loop {
                    match tokio::time::timeout(
                        tokio::time::Duration::from_millis(1500),
                        stream.read(&mut temp_buf)
                    ).await {
                        Ok(Ok(0)) => {
                            println!("[auth-server] Stream EOF reached.");
                            break;
                        },
                        Ok(Ok(n)) => {
                            request_bytes.extend_from_slice(&temp_buf[..n]);
                            
                            if headers_length == 0 {
                                if let Some(pos) = request_bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                                    headers_length = pos + 4;
                                    let headers_str = String::from_utf8_lossy(&request_bytes[..headers_length]);
                                    is_post = headers_str.starts_with("POST ");
                                    
                                    for line in headers_str.lines() {
                                        if line.to_lowercase().starts_with("content-length:") {
                                            if let Some(val) = line.split(':').nth(1) {
                                                content_length = val.trim().parse::<usize>().unwrap_or(0);
                                            }
                                        }
                                    }
                                    println!("[auth-server] Headers read. is_post: {}, Content-Length: {}", is_post, content_length);
                                }
                            }
                            
                            if headers_length > 0 {
                                if is_post {
                                    let body_len = request_bytes.len() - headers_length;
                                    if body_len >= content_length {
                                        println!("[auth-server] Read entire POST body ({} >= {} bytes). Breaking.", body_len, content_length);
                                        break;
                                    }
                                } else {
                                    println!("[auth-server] Read GET/OPTIONS headers. Breaking.");
                                    break;
                                }
                            }
                            
                            if request_bytes.len() > 1024 * 1024 { // 1 MB limit
                                println!("[auth-server] Request exceeds 1MB limit. Breaking.");
                                break;
                            }
                        }
                        Ok(Err(e)) => {
                            println!("[auth-server] Stream read error: {}", e);
                            break;
                        }
                        Err(_) => {
                            println!("[auth-server] Timeout reading next chunk. headers_length: {}, is_post: {}, content_length: {}, current_len: {}", 
                                     headers_length, is_post, content_length, request_bytes.len());
                            break;
                        }
                    }
                }
                
                let request = String::from_utf8_lossy(&request_bytes).to_string();
                let first_line = request.lines().next().unwrap_or("");
                let method = first_line.split_whitespace().next().unwrap_or("");
                let path_and_query = first_line.split_whitespace().nth(1).unwrap_or("");
                let (req_path, query_str) = if let Some(q_pos) = path_and_query.find('?') {
                    (&path_and_query[..q_pos], &path_and_query[q_pos+1..])
                } else {
                    (path_and_query, "")
                };
                println!("[auth-server] Method: '{}', Path: '{}'", method, req_path);

                // URL decode helper
                fn url_decode(s: &str) -> String {
                    let mut out = String::new();
                    let bytes = s.as_bytes();
                    let mut i = 0;
                    while i < bytes.len() {
                        if bytes[i] == b'%' && i + 2 < bytes.len() {
                            if let Ok(hex) = std::str::from_utf8(&bytes[i+1..i+3]) {
                                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                                    out.push(byte as char); i += 3; continue;
                                }
                            }
                        } else if bytes[i] == b'+' {
                            out.push(' '); i += 1; continue;
                        }
                        out.push(bytes[i] as char); i += 1;
                    }
                    out
                }
                fn get_param(qs: &str, key: &str) -> Option<String> {
                    for part in qs.split('&') {
                        let mut kv = part.splitn(2, '=');
                        if kv.next() == Some(key) {
                            return Some(url_decode(kv.next().unwrap_or("")));
                        }
                    }
                    None
                }

                // CORS preflight
                if method == "OPTIONS" {
                    let r = "HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n";
                    let _ = stream.write_all(r.as_bytes()).await;
                    println!("[auth-server] Answered OPTIONS preflight");
                    continue;
                }

                // /callback вЂ” primary OAuth redirect from browser
                if req_path == "/callback" || req_path.starts_with("/callback") {
                    // Check for error
                    if let Some(err) = get_param(query_str, "error").or_else(|| get_param(query_str, "error_description")) {
                        println!("[auth-server] OAuth error: {}", err);
                        let body = format!("<html><body style='background:#05050a;color:#fff;font-family:sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0'><div style='text-align:center;padding:40px;background:rgba(10,10,18,.6);border-radius:24px;max-width:400px'><h2>Ошибка входа</h2><p style='color:#fca5a5'>{}</p><p style='color:#9ca3af'>Закройте это окно и попробуйте ещё раз.</p></div></body></html>", err);
                        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
                        let _ = stream.write_all(resp.as_bytes()).await;
                        continue;
                    }

                    // Extract auth code (PKCE flow) вЂ” this is the primary path
                    if let Some(c) = get_param(query_str, "code") {
                        code = c;
                        println!("[auth-server] Got PKCE auth code (len: {}). Emitting oauth-login...", code.len());
                    }
                    // Fallback: implicit flow with access_token in query
                    if let Some(at) = get_param(query_str, "access_token") {
                        access_token = at;
                        refresh_token = get_param(query_str, "refresh_token").unwrap_or_default();
                        println!("[auth-server] Got access_token in query (len: {})", access_token.len());
                    }

                    if !code.is_empty() || !access_token.is_empty() {
                        // Send success page to browser
                        let success = r#"<html><head><meta charset='UTF-8'><title>Вход выполнен</title><style>body{background:#05050a;color:#f3f4f6;font-family:'Inter',sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}.c{background:rgba(10,10,18,.6);border:1px solid rgba(255,255,255,.08);border-radius:24px;padding:48px 40px;text-align:center;max-width:420px;width:90%}.i{width:64px;height:64px;background:rgba(16,185,129,.1);border:1px solid rgba(16,185,129,.2);border-radius:50%;display:flex;align-items:center;justify-content:center;margin:0 auto 24px;color:#10b981;font-size:28px;animation:s .5s cubic-bezier(.34,1.56,.64,1) forwards}@keyframes s{from{transform:scale(0)}to{transform:scale(1)}}p{color:#9ca3af;font-size:15px;line-height:1.6;margin:0 0 16px}</style></head><body><div class='c'><div class='i'>✓</div><h2>Вход выполнен!</h2><p>Вы успешно авторизовались. Это окно браузера можно закрыть.</p><p style='font-size:13px;border-top:1px solid rgba(255,255,255,.08);padding-top:16px'>Вернитесь в Minecraft P2P Connector.</p></div></body></html>"#;
                        let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", success.len(), success);
                        let _ = stream.write_all(resp.as_bytes()).await;

                        let payload = OAuthPayload {
                            access_token: if access_token.is_empty() { None } else { Some(access_token.clone()) },
                            refresh_token: if refresh_token.is_empty() { None } else { Some(refresh_token.clone()) },
                            code: if code.is_empty() { None } else { Some(code.clone()) },
                        };
                        if let Err(e) = app_clone.emit("oauth-login", payload) {
                            eprintln!("[auth-server] ERROR emitting oauth-login: {}", e);
                        }
                        println!("[auth-server] oauth-login emitted вЂ” server stopping.");
                        break;
                    }

                    // No code/token in query вЂ” serve a page that reads hash fragment (implicit fallback)
                    let hash_page = r#"<html><head><meta charset='UTF-8'><style>body{background:#05050a;color:#fff;font-family:sans-serif;display:flex;align-items:center;justify-content:center;height:100vh;margin:0}.s{border:3px solid rgba(255,255,255,.05);border-top:3px solid #3b82f6;border-radius:50%;width:48px;height:48px;animation:sp 1s linear infinite;margin:0 auto 24px}@keyframes sp{0%{transform:rotate(0deg)}100%{transform:rotate(360deg)}}.c{background:rgba(10,10,18,.6);border:1px solid rgba(255,255,255,.08);border-radius:24px;padding:48px 40px;text-align:center;max-width:420px}p{color:#9ca3af}</style></head><body><div class='c'><div class='s'></div><h2>Обработка...</h2><p>Пожалуйста, подождите.</p></div><script>const h=window.location.hash;if(h&&h.includes('access_token=')){const p=new URLSearchParams(h.slice(1));const at=p.get('access_token');const rt=p.get('refresh_token')||'';if(at){fetch('http://localhost:14235/token?access_token='+encodeURIComponent(at)+'&refresh_token='+encodeURIComponent(rt)).catch(()=>{});}}</script></body></html>"#;
                    let resp = format!("HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", hash_page.len(), hash_page);
                    let _ = stream.write_all(resp.as_bytes()).await;
                    println!("[auth-server] No code in /callback query вЂ” served hash-detection page");
                    continue;
                }

                // /token fallback for implicit flow (hash fragment via JS fetch)
                if req_path == "/token" || req_path.starts_with("/token") {
                    println!("[auth-server] /token fallback: {}", &query_str[..query_str.len().min(60)]);
                    if method == "POST" && headers_length > 0 {
                        let body_str = &request[headers_length..];
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(body_str) {
                            if let Some(at) = json.get("access_token").and_then(|v| v.as_str()) {
                                access_token = at.to_string();
                            }
                            if let Some(rt) = json.get("refresh_token").and_then(|v| v.as_str()) {
                                refresh_token = rt.to_string();
                            }
                            if let Some(c) = json.get("code").and_then(|v| v.as_str()) {
                                code = c.to_string();
                            }
                        }
                    } else {
                        if let Some(at) = get_param(query_str, "access_token") {
                            access_token = at;
                            refresh_token = get_param(query_str, "refresh_token").unwrap_or_default();
                        }
                        if let Some(c) = get_param(query_str, "code") {
                            code = c;
                        }
                    }
                    let resp = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok";
                    let _ = stream.write_all(resp.as_bytes()).await;
                    if !access_token.is_empty() || !code.is_empty() {
                        let payload = OAuthPayload {
                            access_token: if access_token.is_empty() { None } else { Some(access_token.clone()) },
                            refresh_token: if refresh_token.is_empty() { None } else { Some(refresh_token.clone()) },
                            code: if code.is_empty() { None } else { Some(code.clone()) },
                        };
                        if let Err(e) = app_clone.emit("oauth-login", payload) {
                            eprintln!("[auth-server] ERROR emitting oauth-login via /token: {}", e);
                        }
                        println!("[auth-server] oauth-login emitted via /token fallback.");
                        break;
                    }
                    continue;
                }

                // Unknown path
                let resp = "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\nConnection: close\r\n\r\nNot found";
                let _ = stream.write_all(resp.as_bytes()).await;
                println!("[auth-server] 404 for path: '{}'", req_path);
                continue;

            }
        };
        
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(180), server_future).await;
        println!("[auth-server] Server task finished.");
    });
    
    Ok(port)
}
