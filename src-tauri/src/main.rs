#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod models;
mod network;

use network::minecraft::build_preflight_report;
use network::network_swarm::NetworkSwarmManager;
use network::test_server::TestServerManager;
use models::{DiagnosticSnapshot, NetworkStatus, PreflightReport, SwarmBootstrap, TestServerInfo};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

#[derive(Clone)]
struct AppState {
    manager: NetworkSwarmManager,
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
) -> Result<SwarmBootstrap, String> {
    state
        .manager
        .start_hosting(app, room_name, password, local_port)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn stop_hosting(state: State<'_, AppState>) -> Result<(), String> {
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
) -> Result<(), String> {
    state
        .manager
        .connect_to_peer(app, peer_id, peer_addrs)
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
    })
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "minecraft_p2p_connector=info,libp2p=warn".into()),
        )
        .init();

    tauri::Builder::default()
        .manage(AppState {
            manager: NetworkSwarmManager::new(),
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
            run_preflight_and_store,
            start_test_server,
            stop_test_server,
            export_diagnostics_snapshot
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
