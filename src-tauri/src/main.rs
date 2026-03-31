#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod cert;
mod models;
mod network;
mod signaling;

use models::NetworkStatus;
use network::manager::NetworkManager;
use tauri::{AppHandle, State};

#[derive(Clone)]
struct AppState {
    manager: NetworkManager,
}

#[tauri::command]
async fn start_hosting(
    app: AppHandle,
    state: State<'_, AppState>,
    room_name: String,
    password: Option<String>,
    local_port: u16,
) -> Result<String, String> {
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
    peer_addr: String,
    peer_id: Option<String>,
) -> Result<(), String> {
    state
        .manager
        .connect_to_peer(app, peer_addr, peer_id)
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
        })
        .invoke_handler(tauri::generate_handler![
            start_hosting,
            stop_hosting,
            connect_to_peer,
            kick_peer,
            get_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
