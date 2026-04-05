#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod cert;
mod diagnostics;
mod models;
mod network;
mod signaling;

use diagnostics::DiagnosticsStore;
use models::{
    CloudflareRuntimeInfo, DiagnosticSnapshot, NetworkChecks, NetworkStatus, PreflightReport,
    SwarmBootstrap, TestServerInfo, UserProfile, YggstackRuntimeInfo,
};
use network::{
    cloudflare::CloudflareConfig,
    cloudflare_rtc::CloudflareRtcManager,
    manager::NetworkManager,
    minecraft::build_preflight_report,
    selfcheck::run_network_self_check,
    test_server::{probe_test_server, TestServerManager},
    yggstack::YggstackManager,
};
use std::{
    str::FromStr,
    sync::{Arc, Mutex as StdMutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::MenuEvent,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, State,
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tokio::sync::{Mutex, RwLock};

#[derive(Clone)]
struct AppState {
    manager: NetworkManager,
    cloudflare: CloudflareRtcManager,
    yggstack: YggstackManager,
    test_server: TestServerManager,
    diagnostics: DiagnosticsStore,
    last_preflight: Arc<Mutex<Option<PreflightReport>>>,
    overlay_shortcut: Arc<StdMutex<String>>,
    profile: Arc<StdMutex<UserProfile>>,
}

#[tauri::command]
async fn start_hosting(
    app: AppHandle,
    state: State<'_, AppState>,
    room_name: String,
    password: Option<String>,
    local_port: u16,
    use_cloudflare: bool,
) -> Result<SwarmBootstrap, String> {
    let cloudflare_runtime = if use_cloudflare {
        state.cloudflare.runtime_info().await
    } else {
        CloudflareRuntimeInfo::default()
    };
    let effective_cloudflare = use_cloudflare && cloudflare_runtime.ready;
    let public_addr = state
        .manager
        .start_hosting(app, room_name, password, local_port, effective_cloudflare)
        .await
        .map_err(|error| format!("{error:#}"))?;

    if use_cloudflare && !cloudflare_runtime.ready {
        state
            .manager
            .shared_status()
            .write()
            .await
            .logs
            .insert(
                0,
                format!(
                    "Cloudflare fallback requested but runtime is not ready: {}",
                    cloudflare_runtime.note
                ),
            );
    }

    Ok(SwarmBootstrap {
        peer_id: String::new(),
        listen_addrs: vec![normalize_socket_addr_to_multiaddr(&public_addr)],
        relay_addrs: Vec::new(),
        nat_status: if effective_cloudflare {
            "quic-direct+cloudflare-ready".into()
        } else {
            "quic-direct".into()
        },
        local_game_port: Some(local_port),
        transport_preference: Some(if effective_cloudflare {
            "cloudflare".into()
        } else {
            "direct".into()
        }),
    })
}

#[tauri::command]
async fn stop_hosting(state: State<'_, AppState>) -> Result<(), String> {
    state.cloudflare.abort_all().await;
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
    allow_relay_fallback: Option<bool>,
) -> Result<(), String> {
    let peer_addr = peer_addrs
        .iter()
        .find_map(|value| multiaddr_or_socket_to_socket(value))
        .ok_or_else(|| "Не удалось извлечь socket address из peerAddrs".to_string())?;

    state
        .manager
        .connect_to_peer(
            app,
            peer_addr,
            (!peer_id.trim().is_empty()).then_some(peer_id),
            if allow_relay_fallback.unwrap_or(true) {
                relay_session_id
            } else {
                None
            },
        )
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn start_relay_fallback(
    app: AppHandle,
    state: State<'_, AppState>,
    peer_id: String,
    peer_addrs: Vec<String>,
    relay_session_id: String,
) -> Result<(), String> {
    connect_to_peer(
        app,
        state,
        peer_id,
        peer_addrs,
        Some(relay_session_id),
        Some(true),
    )
    .await
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
async fn probe_test_server_command(port: u16, payload: Option<String>) -> Result<String, String> {
    probe_test_server(
        port,
        payload.unwrap_or_else(|| "diagnostic-ping".into()),
    )
    .await
    .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn get_cloudflare_runtime_info(
    state: State<'_, AppState>,
) -> Result<CloudflareRuntimeInfo, String> {
    Ok(state.cloudflare.runtime_info().await)
}

#[tauri::command]
async fn get_yggstack_runtime_info(
    state: State<'_, AppState>,
) -> Result<YggstackRuntimeInfo, String> {
    Ok(state.yggstack.runtime_info().await)
}

#[tauri::command]
async fn prepare_yggstack_runtime(
    state: State<'_, AppState>,
) -> Result<YggstackRuntimeInfo, String> {
    state
        .yggstack
        .prepare_runtime()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn start_yggstack_sidecar(
    state: State<'_, AppState>,
) -> Result<YggstackRuntimeInfo, String> {
    state
        .yggstack
        .start_sidecar()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn stop_yggstack_sidecar(
    state: State<'_, AppState>,
) -> Result<YggstackRuntimeInfo, String> {
    state
        .yggstack
        .stop_sidecar()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn retry_yggstack_peers(
    state: State<'_, AppState>,
) -> Result<YggstackRuntimeInfo, String> {
    state
        .yggstack
        .retry_peers()
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn cloudflare_create_offer(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    peer_addr: String,
) -> Result<String, String> {
    state
        .cloudflare
        .create_client_offer(app, session_id, peer_addr)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn cloudflare_accept_offer(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    offer_json: String,
    peer_addr: String,
) -> Result<String, String> {
    let local_port = state
        .manager
        .get_status()
        .await
        .local_game_port
        .ok_or_else(|| "Локальный игровой порт хоста не найден".to_string())?;
    state
        .cloudflare
        .accept_host_offer(app, session_id, offer_json, local_port, peer_addr)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn cloudflare_finish_client_answer(
    state: State<'_, AppState>,
    session_id: String,
    answer_json: String,
) -> Result<(), String> {
    state
        .cloudflare
        .finish_client_answer(session_id, answer_json)
        .await
        .map_err(|error| format!("{error:#}"))
}

#[tauri::command]
async fn cloudflare_abort_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    state.cloudflare.abort_session(&session_id).await;
    Ok(())
}

#[tauri::command]
async fn run_network_self_check_command(
    state: State<'_, AppState>,
) -> Result<NetworkChecks, String> {
    let checks = run_network_self_check(
        state
            .cloudflare
            .runtime_info()
            .await
            .credential_endpoint
            .as_deref(),
    )
    .await;
    state.diagnostics.set_network_checks(checks.clone()).await;
    Ok(checks)
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
    let diagnostics = state.diagnostics.snapshot().await;
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
        network_checks: diagnostics.network_checks,
        direct_attempt: diagnostics.direct_attempt,
        cloudflare_attempt: diagnostics.cloudflare_attempt,
        yggstack_runtime: Some(state.yggstack.runtime_info().await),
        selected_transport: diagnostics.selected_transport,
    })
}

#[tauri::command]
fn get_user_profile(state: State<'_, AppState>) -> Result<UserProfile, String> {
    Ok(state
        .profile
        .lock()
        .map_err(|_| "profile mutex poisoned".to_string())?
        .clone())
}

#[tauri::command]
fn save_user_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile: UserProfile,
) -> Result<(), String> {
    {
        let mut current = state
            .profile
            .lock()
            .map_err(|_| "profile mutex poisoned".to_string())?;
        *current = profile.clone();
    }
    set_overlay_shortcut(app, state, profile.overlay_shortcut)?;
    Ok(())
}

#[tauri::command]
fn show_overlay(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.show().map_err(|error| error.to_string())?;
    let _ = window.unminimize();
    let _ = window.set_focus();
    Ok(())
}

#[tauri::command]
fn hide_overlay(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window not found".to_string())?;
    window.hide().map_err(|error| error.to_string())
}

#[tauri::command]
fn toggle_overlay(app: AppHandle) -> Result<(), String> {
    toggle_overlay_impl(&app).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_overlay_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    shortcut: String,
) -> Result<(), String> {
    let shortcut = normalize_shortcut(&shortcut);
    let parsed = Shortcut::from_str(&shortcut).map_err(|error| error.to_string())?;
    let global = app.global_shortcut();
    let previous = {
        let mut guard = state
            .overlay_shortcut
            .lock()
            .map_err(|_| "shortcut mutex poisoned".to_string())?;
        let previous = guard.clone();
        *guard = shortcut.clone();
        previous
    };

    if !previous.trim().is_empty() {
        if let Ok(parsed_previous) = Shortcut::from_str(&previous) {
            let _ = global.unregister(parsed_previous);
        }
    }
    global.register(parsed).map_err(|error| error.to_string())?;
    Ok(())
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "minecraft_p2p_connector=info,quinn=warn".into()),
        )
        .init();

    let shared_status = Arc::new(RwLock::new(NetworkStatus {
        signaling_server: "Ably Presence + Channels".into(),
        ..Default::default()
    }));
    let diagnostics = DiagnosticsStore::new();
    let manager = NetworkManager::new_with_shared(shared_status.clone(), diagnostics.clone());
    let cloudflare = CloudflareRtcManager::new(
        shared_status,
        diagnostics.clone(),
        CloudflareConfig::from_env(),
    );
    let yggstack = YggstackManager::from_env();
    let overlay_shortcut = Arc::new(StdMutex::new(String::from("SHIFT+TAB")));
    let profile = Arc::new(StdMutex::new(UserProfile {
        nickname: String::new(),
        avatar_data_url: None,
        theme: "oled".into(),
        language: "ru".into(),
        overlay_shortcut: "SHIFT+TAB".into(),
    }));

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new().with_handler({
                let overlay_shortcut = overlay_shortcut.clone();
                move |app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    let current = overlay_shortcut
                        .lock()
                        .map(|value| value.clone())
                        .unwrap_or_else(|_| "SHIFT+TAB".into());
                    if Shortcut::from_str(&current).ok().as_ref() == Some(shortcut) {
                        let _ = toggle_overlay_impl(app);
                    }
                }
            })
            .build(),
        )
        .manage(AppState {
            manager,
            cloudflare,
            yggstack,
            test_server: TestServerManager::new(),
            diagnostics,
            last_preflight: Arc::new(Mutex::new(None)),
            overlay_shortcut,
            profile,
        })
        .setup(|app| {
            configure_tray(app)?;
            let default_shortcut = Shortcut::new(Some(Modifiers::SHIFT), Code::Tab);
            app.global_shortcut().register(default_shortcut)?;
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.hide();
                let _ = window.set_skip_taskbar(true);
                let _ = window.set_always_on_top(true);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_hosting,
            stop_hosting,
            connect_to_peer,
            start_relay_fallback,
            kick_peer,
            get_status,
            run_preflight,
            run_preflight_and_store,
            start_test_server,
            stop_test_server,
            probe_test_server_command,
            get_cloudflare_runtime_info,
            get_yggstack_runtime_info,
            prepare_yggstack_runtime,
            start_yggstack_sidecar,
            stop_yggstack_sidecar,
            retry_yggstack_peers,
            cloudflare_create_offer,
            cloudflare_accept_offer,
            cloudflare_finish_client_answer,
            cloudflare_abort_session,
            run_network_self_check_command,
            export_diagnostics_snapshot,
            get_user_profile,
            save_user_profile,
            show_overlay,
            hide_overlay,
            toggle_overlay,
            set_overlay_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn configure_tray(app: &mut tauri::App) -> tauri::Result<()> {
    let open = MenuItemBuilder::with_id("open_overlay", "Открыть overlay").build(app)?;
    let restart = MenuItemBuilder::with_id("restart_app", "Перезапустить").build(app)?;
    let quit = MenuItemBuilder::with_id("quit_app", "Выход").build(app)?;
    let menu = MenuBuilder::new(app).items(&[&open, &restart, &quit]).build()?;

    let mut builder = TrayIconBuilder::new()
        .menu(&menu)
        .on_menu_event(|app, event: MenuEvent| match event.id().as_ref() {
            "open_overlay" => {
                let _ = toggle_overlay_impl(app);
            }
            "restart_app" => app.restart(),
            "quit_app" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray: &TrayIcon<_>, event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = toggle_overlay_impl(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder.build(app)?;
    Ok(())
}

fn toggle_overlay_impl(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };
    if window.is_visible().map_err(|error| error.to_string())? {
        window.hide().map_err(|error| error.to_string())?;
    } else {
        window.show().map_err(|error| error.to_string())?;
        let _ = window.unminimize();
        let _ = window.center();
        let _ = window.set_focus();
    }
    Ok(())
}

fn normalize_shortcut(value: &str) -> String {
    value.trim().replace("Shift", "SHIFT").replace("Tab", "TAB")
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
