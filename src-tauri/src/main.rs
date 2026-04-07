#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod cert;
mod models;
mod network;
mod signaling;

use network::minecraft::{build_preflight_report, detect_lan_port_from_logs};
use network::manager::NetworkManager;
use network::geyser::GeyserManager;
use network::test_server::{probe_test_server, TestServerManager};
use models::{
    DiagnosticSnapshot, LanPortDetection, NetworkStatus, PreflightReport, SwarmBootstrap,
    TestServerInfo,
};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

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
) -> Result<SwarmBootstrap, String> {
    let room_name_for_geyser = room_name.clone();
    let public_addr = state
        .manager
        .start_hosting(app, room_name, password, local_port)
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
            kick_peer,
            get_status,
            run_preflight,
            detect_lan_port,
            run_preflight_and_store,
            start_test_server,
            stop_test_server,
            probe_test_server_command,
            export_diagnostics_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn normalize_socket_addr_to_multiaddr(value: &str) -> String {
    if let Ok(socket) = value.parse::<std::net::SocketAddr>() {
        match socket.ip() {
            std::net::IpAddr::V4(ip) => format!("/ip4/{ip}/tcp/{}", socket.port()),
            std::net::IpAddr::V6(ip) => format!("/ip6/{ip}/tcp/{}", socket.port()),
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
    if parts.len() >= 5 && parts[1] == "ip4" && parts[3] == "tcp" {
        return Some(format!("{}:{}", parts[2], parts[4]));
    }
    if parts.len() >= 5 && parts[1] == "ip6" && parts[3] == "tcp" {
        return Some(format!("[{}]:{}", parts[2], parts[4]));
    }

    None
}

fn derive_bedrock_public_endpoint(public_addr: &str, bedrock_port: u16) -> Option<String> {
    let trimmed = public_addr.trim();
    let socket = trimmed.parse::<std::net::SocketAddr>().ok()?;
    Some(match socket.ip() {
        std::net::IpAddr::V4(ip) => format!("{ip}:{bedrock_port}"),
        std::net::IpAddr::V6(ip) => format!("[{ip}]:{bedrock_port}"),
    })
}

