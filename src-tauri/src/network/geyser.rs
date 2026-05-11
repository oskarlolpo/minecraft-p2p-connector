use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::Arc,
    time::Duration,
};

use anyhow::{anyhow, Context, Result};
use tauri::{AppHandle, Emitter};
use tokio::{sync::Mutex, task};

use crate::models::GeyserRuntimeInfo;

const DEFAULT_BEDROCK_PORT: u16 = 19132;
const GEYSER_JAR_NAME: &str = "Geyser-Standalone.jar";

#[derive(Clone, Default)]
pub struct GeyserManager {
    inner: Arc<Mutex<GeyserState>>,
}

#[derive(Default)]
struct GeyserState {
    child: Option<Child>,
    info: GeyserRuntimeInfo,
}

impl GeyserManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn current_info(&self) -> Option<GeyserRuntimeInfo> {
        let state = self.inner.lock().await;
        if !state.info.enabled
            && state.info.jar_path.is_none()
            && state.info.runtime_dir.is_none()
            && state.info.note.is_none()
            && state.info.last_error.is_none()
        {
            None
        } else {
            Some(state.info.clone())
        }
    }

    pub async fn start(
        &self,
        app: AppHandle,
        local_java_port: u16,
        room_name: &str,
        bedrock_port: Option<u16>,
        bedrock_public_endpoint: Option<String>,
    ) -> Result<GeyserRuntimeInfo> {
        self.stop().await?;

        let resolved_bedrock_port = bedrock_port.unwrap_or(DEFAULT_BEDROCK_PORT);
        let runtime_dir = resolve_runtime_dir()?;
        fs::create_dir_all(&runtime_dir)
            .with_context(|| format!("failed to create Geyser runtime dir {}", runtime_dir.display()))?;

        let source_jar = resolve_source_jar()?;
        let runtime_jar = runtime_dir.join(GEYSER_JAR_NAME);
        if runtime_jar != source_jar {
            fs::copy(&source_jar, &runtime_jar).with_context(|| {
                format!(
                    "failed to copy Geyser jar from {} to {}",
                    source_jar.display(),
                    runtime_jar.display()
                )
            })?;
        }

        let config_path = runtime_dir.join("config.yml");
        let log_path = runtime_dir.join("geyser.log");
        write_config(&config_path, local_java_port, resolved_bedrock_port, room_name)?;

        // We'll pipe stdout and stderr to a file AND stream it to the UI
        let java_path = super::java_downloader::ensure_java_ready(&app).await.unwrap_or_else(|_| super::java_downloader::resolve_java_binary());
        
        let firewall_result = ensure_firewall_rule(resolved_bedrock_port);
        let firewall_rule_name = firewall_result.as_ref().ok().cloned();
        let firewall_note = firewall_result.err().map(|error| error.to_string());

        let mut child = spawn_geyser_process(
            &java_path,
            &runtime_jar,
            &runtime_dir,
            &config_path,
        )?;

        let stdout = child.stdout.take().context("failed to take Geyser stdout")?;
        let stderr = child.stderr.take().context("failed to take Geyser stderr")?;

        // Spawn log forwarding tasks
        let (done_tx, mut done_rx) = tokio::sync::mpsc::channel::<()>(1);
        let app_stdout = app.clone();
        task::spawn_blocking(move || {
            let reader = BufReader::new(stdout);
            let mut sent_done = false;
            for line in reader.lines() {
                if let Ok(l) = line {
                    if !sent_done && l.contains("Done (") {
                        let _ = done_tx.blocking_send(());
                        sent_done = true;
                    }
                    let _ = app_stdout.emit("geyser-log", l.clone());
                }
            }
        });

        let app_stderr = app.clone();
        task::spawn_blocking(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(l) = line {
                    let _ = app_stderr.emit("geyser-log", format!("[ERROR] {}", l));
                }
            }
        });

        let mut state = self.inner.lock().await;
        state.child = Some(child);
        state.info = GeyserRuntimeInfo {
            enabled: true,
            running: true,
            java_path: Some(java_path),
            jar_path: Some(runtime_jar.display().to_string()),
            runtime_dir: Some(runtime_dir.display().to_string()),
            config_path: Some(config_path.display().to_string()),
            log_path: Some(log_path.display().to_string()),
            bedrock_port: Some(resolved_bedrock_port),
            bedrock_public_endpoint,
            firewall_rule_name,
            note: Some(match &firewall_note {
                Some(warning) => format!(
                    "Bedrock bridge is active on UDP {resolved_bedrock_port} and forwards to Java localhost:{local_java_port}. Firewall warning: {warning}"
                ),
                None => format!(
                    "Bedrock bridge is active on UDP {resolved_bedrock_port} and forwards to Java localhost:{local_java_port}."
                ),
            }),
            last_error: None,
        };

        drop(state);
        let _ = tokio::time::timeout(Duration::from_secs(15), done_rx.recv()).await;

        let mut state = self.inner.lock().await;
        if let Some(child) = state.child.as_mut() {
            if let Some(status) = child.try_wait().context("failed to query Geyser process status")? {
                state.child = None;
                state.info.running = false;
                state.info.last_error = Some(format!("Geyser exited early with status {status}."));
                return Err(anyhow!(
                    state
                        .info
                        .last_error
                        .clone()
                        .unwrap_or_else(|| "Geyser exited unexpectedly.".into())
                ));
            }
        }

        Ok(state.info.clone())
    }

    pub async fn stop(&self) -> Result<()> {
        let child = {
            let mut state = self.inner.lock().await;
            state.info.running = false;
            state.child.take()
        };

        if let Some(mut child) = child {
            task::spawn_blocking(move || -> Result<()> {
                child.kill().ok();
                let _ = child.wait();
                Ok(())
            })
            .await
            .context("failed to await Geyser shutdown task")??;
        }

        Ok(())
    }
}

fn spawn_geyser_process(
    java_path: &str,
    jar_path: &Path,
    runtime_dir: &Path,
    config_path: &Path,
) -> Result<Child> {
    let mut command = Command::new(java_path);
    command
        .arg("-Dfile.encoding=UTF-8")
        .arg("-jar")
        .arg(jar_path)
        .arg("--nogui")
        .arg("--config")
        .arg(config_path)
        .current_dir(runtime_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW | DETACHED_PROCESS
        command.creation_flags(0x0800_0000 | 0x0000_0008);
    }

    command
        .spawn()
        .with_context(|| format!("failed to start Geyser using {}", jar_path.display()))
}

fn resolve_runtime_dir() -> Result<PathBuf> {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(local_app_data)
            .join("MinecraftP2PConnector")
            .join("geyser"));
    }

    Ok(std::env::temp_dir().join("MinecraftP2PConnector").join("geyser"))
}

fn resolve_source_jar() -> Result<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(explicit) = std::env::var_os("MC_GEYSER_JAR") {
        candidates.push(PathBuf::from(explicit));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join("resources").join("geyser").join(GEYSER_JAR_NAME));
            candidates.push(exe_dir.join("geyser").join(GEYSER_JAR_NAME));
            candidates.push(exe_dir.join(GEYSER_JAR_NAME));
        }
    }

    candidates.push(PathBuf::from(r"G:\oskarlolpo project\minecraftjava\01_Active\p2p\src-tauri\resources\geyser").join(GEYSER_JAR_NAME));
    candidates.push(
        PathBuf::from(r"G:\oskarlolpo project\minecraftjava\03_Integrations\Geyser-master\bootstrap\standalone\build\libs")
            .join(GEYSER_JAR_NAME),
    );

    if let Some(found) = candidates.into_iter().find(|path| path.exists()) {
        return Ok(found);
    }

    let libs_dir = PathBuf::from(r"G:\oskarlolpo project\minecraftjava\03_Integrations\Geyser-master\bootstrap\standalone\build\libs");
    if libs_dir.exists() {
        if let Some(found) = fs::read_dir(&libs_dir)
            .ok()
            .into_iter()
            .flat_map(|entries| entries.filter_map(Result::ok))
            .map(|entry| entry.path())
            .find(|path| {
                path.extension().and_then(|value| value.to_str()) == Some("jar")
                    && path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .map(|value| value.starts_with("Geyser-Standalone"))
                        .unwrap_or(false)
            })
        {
            return Ok(found);
        }
    }

    Err(anyhow!(
        "Geyser-Standalone.jar not found. Build Geyser standalone first or set MC_GEYSER_JAR."
    ))
}

fn resolve_java_binary() -> String {
    if let Some(java_home) = std::env::var_os("JAVA_HOME") {
        let candidate = PathBuf::from(java_home).join("bin").join("java.exe");
        if candidate.exists() {
            return candidate.display().to_string();
        }
    }
    "java".into()
}

fn write_config(config_path: &Path, local_java_port: u16, bedrock_port: u16, room_name: &str) -> Result<()> {
    let motd = sanitize_yaml_text(room_name);
    let yaml = format!(
        r#"# Auto-generated by Minecraft P2P Connector
bedrock:
  address: 0.0.0.0
  port: {bedrock_port}
  clone-remote-port: false
java:
  address: 127.0.0.1
  port: {local_java_port}
  auth-type: offline
  forward-hostname: false
motd:
  primary-motd: "{motd}"
  secondary-motd: "Bedrock bridge via Minecraft P2P Connector"
  passthrough-motd: true
  max-players: 100
  passthrough-player-counts: true
  integrated-ping-passthrough: true
  ping-passthrough-interval: 3
gameplay:
  server-name: "{motd}"
  show-cooldown: crosshair
  command-suggestions: true
  show-coordinates: true
  disable-bedrock-scaffolding: false
  nether-roof-workaround: true
  emotes-enabled: true
  unusable-space-block: minecraft:barrier
  enable-custom-content: true
  force-resource-packs: false
  enable-integrated-pack: true
  forward-player-ping: true
  xbox-achievements-enabled: false
  max-visible-custom-skulls: 128
  custom-skull-render-distance: 32
default-locale: system
log-player-ip-addresses: true
saved-user-logins: []
pending-authentication-timeout: 120
notify-on-new-bedrock-update: true
advanced:
  cache-images: 0
  scoreboard-packet-threshold: 20
  add-team-suggestions: true
  resource-pack-urls: []
  floodgate-key-file: key.pem
  java:
    use-haproxy-protocol: false
    use-direct-connection: false
    disable-compression: false
  bedrock:
    broadcast-port: 0
    compression-level: 6
    use-haproxy-protocol: false
    haproxy-protocol-whitelisted-ips: []
    use-waterdogpe-forwarding: false
    mtu: 1400
  validate-bedrock-login: false
  # Global API for Bedrock skins support
  api:
    base-url: "https://api.geysermc.org"
    skin-refresh-interval: 1440
enable-metrics: false
debug-mode: false
config-version: 5
"#
    );

    fs::write(config_path, yaml)
        .with_context(|| format!("failed to write Geyser config to {}", config_path.display()))
}

fn sanitize_yaml_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn tail_lines(text: &str, limit: usize) -> String {
    let lines = text.lines().collect::<Vec<_>>();
    let start = lines.len().saturating_sub(limit);
    lines[start..].join("\n")
}

fn ensure_firewall_rule(port: u16) -> Result<String> {
    #[cfg(target_os = "windows")]
    {
        let rule_name = format!("Minecraft P2P Connector Bedrock UDP {port}");
        let _ = Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "delete",
                "rule",
                &format!("name={rule_name}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        let status = Command::new("netsh")
            .args([
                "advfirewall",
                "firewall",
                "add",
                "rule",
                &format!("name={rule_name}"),
                "dir=in",
                "action=allow",
                "protocol=UDP",
                &format!("localport={port}"),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to invoke netsh for Bedrock firewall rule")?;

        if status.success() {
            return Ok(rule_name);
        }

        return Err(anyhow!(
            "failed to create a Windows Firewall rule for the Bedrock UDP port"
        ));
    }

    #[allow(unreachable_code)]
    Err(anyhow!("firewall rule management is only implemented on Windows"))
}
