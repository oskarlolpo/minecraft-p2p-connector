mod cert;
mod models;
mod network;
mod signaling;

use models::NetworkStatus;
use network::manager::NetworkManager;
use tauri::State;

#[derive(Clone)]
struct AppState {
    manager: NetworkManager,
}

#[tauri::command]
async fn start_hosting(state: State<'_, AppState>) -> Result<String, String> {
    state
        .manager
        .start_hosting()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn connect_to_host(state: State<'_, AppState>, room_code: String) -> Result<(), String> {
    state
        .manager
        .connect_to_host(room_code)
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
            connect_to_host,
            get_status
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
