use std::{
    env,
    ffi::CStr,
    fs::{self, File},
    os::raw::c_char,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use tokio::task;

use crate::models::YggstackRuntimeInfo;

#[derive(Clone)]
pub struct YggstackManager {
    config: YggstackConfig,
    process: Arc<Mutex<Option<ManagedProcess>>>,
}

#[derive(Clone)]
struct YggstackConfig {
    source_dir: PathBuf,
    runtime_dir: PathBuf,
    binary_path: PathBuf,
    config_path: PathBuf,
    log_path: PathBuf,
}

struct ManagedProcess {
    child: Child,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EmbeddedBridgeStatus {
    running: bool,
    public_key: Option<String>,
    address: Option<String>,
    subnet: Option<String>,
    error: Option<String>,
}

impl YggstackManager {
    pub fn from_env() -> Self {
        let source_dir = env::var("MC_YGGSTACK_SOURCE_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(r"G:\minecraftjava\newrepo\yggstack-develop"));

        let runtime_root = env::var("MC_YGGSTACK_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| default_runtime_dir());

        let runtime_dir = runtime_root.join("yggstack");
        let binary_path = runtime_dir.join(yggstack_binary_name());
        let config_path = runtime_dir.join("yggstack.autogen.conf");
        let log_path = runtime_dir.join("yggstack.log");

        Self {
            config: YggstackConfig {
                source_dir,
                runtime_dir,
                binary_path,
                config_path,
                log_path,
            },
            process: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn runtime_info(&self) -> YggstackRuntimeInfo {
        if embedded_bridge_available() {
            return self.embedded_runtime_info();
        }

        self.sidecar_runtime_info()
    }

    pub async fn prepare_runtime(&self) -> Result<YggstackRuntimeInfo> {
        if embedded_bridge_available() {
            return Ok(self.embedded_runtime_info());
        }

        self.ensure_runtime_dir()?;
        self.ensure_source_dir()?;
        self.build_binary_if_missing().await?;
        self.generate_config_if_missing().await?;
        Ok(self.sidecar_runtime_info())
    }

    pub async fn start_sidecar(&self) -> Result<YggstackRuntimeInfo> {
        if embedded_bridge_available() {
            embedded_bridge_start()?;
            return Ok(self.embedded_runtime_info());
        }

        self.prepare_runtime().await?;

        if self.refresh_running_flag() {
            return Ok(self.sidecar_runtime_info());
        }

        self.ensure_runtime_dir()?;
        let log_file = File::options()
            .create(true)
            .append(true)
            .open(&self.config.log_path)
            .with_context(|| format!("не удалось открыть лог {}", self.config.log_path.display()))?;
        let err_file = log_file
            .try_clone()
            .context("не удалось клонировать файловый дескриптор лога yggstack")?;

        let child = Command::new(&self.config.binary_path)
            .arg("-autoconf")
            .arg("-logto")
            .arg(&self.config.log_path)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(err_file))
            .spawn()
            .with_context(|| {
                format!(
                    "не удалось запустить yggstack из {}",
                    self.config.binary_path.display()
                )
            })?;

        *self
            .process
            .lock()
            .map_err(|_| anyhow!("mutex yggstack process poisoned"))? = Some(ManagedProcess { child });

        Ok(self.sidecar_runtime_info())
    }

    pub async fn stop_sidecar(&self) -> Result<YggstackRuntimeInfo> {
        if embedded_bridge_available() {
            embedded_bridge_stop()?;
            return Ok(self.embedded_runtime_info());
        }

        if let Some(mut process) = self
            .process
            .lock()
            .map_err(|_| anyhow!("mutex yggstack process poisoned"))?
            .take()
        {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }

        Ok(self.sidecar_runtime_info())
    }

    pub async fn retry_peers(&self) -> Result<YggstackRuntimeInfo> {
        if embedded_bridge_available() {
            embedded_bridge_retry_peers()?;
            return Ok(self.embedded_runtime_info());
        }

        Ok(self.sidecar_runtime_info())
    }

    fn embedded_runtime_info(&self) -> YggstackRuntimeInfo {
        match embedded_bridge_status() {
            Ok(status) => {
                let mut note_parts = Vec::new();
                if let Some(error) = status.error.as_deref() {
                    note_parts.push(format!("embedded bridge error: {error}"));
                }
                if status.running {
                    note_parts.push("Встроенный Yggstack bridge запущен.".into());
                } else {
                    note_parts.push("Встроенный Yggstack bridge готов.".into());
                }
                if let Some(address) = status.address.as_deref() {
                    note_parts.push(format!("Ygg address: {address}"));
                }
                if let Some(public_key) = status.public_key.as_deref() {
                    note_parts.push(format!("Public key: {public_key}"));
                }
                if let Some(subnet) = status.subnet.as_deref() {
                    note_parts.push(format!("Subnet: {subnet}"));
                }

                YggstackRuntimeInfo {
                    ready: status.error.is_none(),
                    running: status.running,
                    source_dir: Some(self.config.source_dir.display().to_string()),
                    runtime_dir: None,
                    binary_path: Some("embedded://yggstackbridge".into()),
                    config_path: None,
                    log_path: None,
                    ygg_public_key: status.public_key.clone(),
                    ygg_address: status.address.clone(),
                    ygg_subnet: status.subnet.clone(),
                    note: note_parts.join(" "),
                }
            }
            Err(error) => YggstackRuntimeInfo {
                ready: false,
                running: false,
                source_dir: Some(self.config.source_dir.display().to_string()),
                runtime_dir: None,
                binary_path: Some("embedded://yggstackbridge".into()),
                config_path: None,
                log_path: None,
                ygg_public_key: None,
                ygg_address: None,
                ygg_subnet: None,
                note: format!("Встроенный Yggstack bridge недоступен: {error:#}"),
            },
        }
    }

    fn sidecar_runtime_info(&self) -> YggstackRuntimeInfo {
        let mut note_parts = Vec::new();
        let source_exists = self.config.source_dir.exists();
        let binary_exists = self.config.binary_path.exists();
        let config_exists = self.config.config_path.exists();
        let running = self.refresh_running_flag();

        if !source_exists {
            note_parts.push(format!(
                "Исходники yggstack не найдены: {}",
                self.config.source_dir.display()
            ));
        }
        if source_exists && !binary_exists {
            note_parts.push("Бинарник yggstack ещё не собран.".into());
        }
        if binary_exists && !config_exists {
            note_parts.push("Конфиг yggstack ещё не сгенерирован.".into());
        }
        if running {
            note_parts.push("Yggstack sidecar запущен.".into());
        }
        if note_parts.is_empty() {
            note_parts.push("Yggstack runtime готов.".into());
        }

        YggstackRuntimeInfo {
            ready: source_exists && binary_exists && config_exists,
            running,
            source_dir: Some(self.config.source_dir.display().to_string()),
            runtime_dir: Some(self.config.runtime_dir.display().to_string()),
            binary_path: Some(self.config.binary_path.display().to_string()),
            config_path: Some(self.config.config_path.display().to_string()),
            log_path: Some(self.config.log_path.display().to_string()),
            ygg_public_key: None,
            ygg_address: None,
            ygg_subnet: None,
            note: note_parts.join(" "),
        }
    }

    fn refresh_running_flag(&self) -> bool {
        let mut guard = match self.process.lock() {
            Ok(guard) => guard,
            Err(_) => return false,
        };

        if let Some(process) = guard.as_mut() {
            match process.child.try_wait() {
                Ok(Some(_)) => {
                    *guard = None;
                    false
                }
                Ok(None) => true,
                Err(_) => {
                    *guard = None;
                    false
                }
            }
        } else {
            false
        }
    }

    fn ensure_runtime_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.config.runtime_dir).with_context(|| {
            format!(
                "не удалось создать runtime каталог {}",
                self.config.runtime_dir.display()
            )
        })
    }

    fn ensure_source_dir(&self) -> Result<()> {
        if self.config.source_dir.exists() {
            return Ok(());
        }
        Err(anyhow!(
            "исходники yggstack не найдены по пути {}",
            self.config.source_dir.display()
        ))
    }

    async fn build_binary_if_missing(&self) -> Result<()> {
        if self.config.binary_path.exists() {
            return Ok(());
        }

        let source_dir = self.config.source_dir.clone();
        let binary_path = self.config.binary_path.clone();
        let runtime_dir = self.config.runtime_dir.clone();

        task::spawn_blocking(move || build_binary(&source_dir, &runtime_dir, &binary_path))
            .await
            .context("сборка yggstack task panicked")??;

        Ok(())
    }

    async fn generate_config_if_missing(&self) -> Result<()> {
        if self.config.config_path.exists() {
            return Ok(());
        }

        let binary_path = self.config.binary_path.clone();
        let config_path = self.config.config_path.clone();

        task::spawn_blocking(move || generate_config(&binary_path, &config_path))
            .await
            .context("генерация конфига yggstack task panicked")??;

        Ok(())
    }
}

fn build_binary(source_dir: &Path, runtime_dir: &Path, binary_path: &Path) -> Result<()> {
    fs::create_dir_all(runtime_dir).with_context(|| {
        format!(
            "не удалось создать runtime каталог для yggstack {}",
            runtime_dir.display()
        )
    })?;

    let status = Command::new("go")
        .arg("build")
        .arg("-o")
        .arg(binary_path)
        .arg("./cmd/yggstack")
        .current_dir(source_dir)
        .status()
        .with_context(|| format!("не удалось запустить go build в {}", source_dir.display()))?;

    if !status.success() {
        return Err(anyhow!(
            "go build завершился с ошибкой при сборке yggstack в {}",
            source_dir.display()
        ));
    }

    Ok(())
}

fn generate_config(binary_path: &Path, config_path: &Path) -> Result<()> {
    let output = Command::new(binary_path)
        .arg("-genconf")
        .arg("-json")
        .output()
        .with_context(|| format!("не удалось запустить {}", binary_path.display()))?;

    if !output.status.success() {
        return Err(anyhow!(
            "yggstack -genconf завершился с ошибкой: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    fs::write(config_path, output.stdout).with_context(|| {
        format!(
            "не удалось записать сгенерированный конфиг yggstack в {}",
            config_path.display()
        )
    })?;

    Ok(())
}

fn default_runtime_dir() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"G:\minecraftjava\p2p\.runtime"))
        .join("MinecraftP2PConnector")
}

fn yggstack_binary_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "yggstack.exe"
    } else {
        "yggstack"
    }
}

#[cfg(embedded_ygg)]
fn embedded_bridge_available() -> bool {
    true
}

#[cfg(not(embedded_ygg))]
fn embedded_bridge_available() -> bool {
    false
}

#[cfg(embedded_ygg)]
fn embedded_bridge_start() -> Result<EmbeddedBridgeStatus> {
    unsafe { call_bridge(ffi::YggstackBridgeStartAutoconf) }
}

#[cfg(not(embedded_ygg))]
fn embedded_bridge_start() -> Result<EmbeddedBridgeStatus> {
    Err(anyhow!("embedded Yggstack bridge is not compiled into this build"))
}

#[cfg(embedded_ygg)]
fn embedded_bridge_status() -> Result<EmbeddedBridgeStatus> {
    unsafe { call_bridge(ffi::YggstackBridgeStatus) }
}

#[cfg(not(embedded_ygg))]
fn embedded_bridge_status() -> Result<EmbeddedBridgeStatus> {
    Err(anyhow!("embedded Yggstack bridge is not compiled into this build"))
}

#[cfg(embedded_ygg)]
fn embedded_bridge_retry_peers() -> Result<EmbeddedBridgeStatus> {
    unsafe { call_bridge(ffi::YggstackBridgeRetryPeers) }
}

#[cfg(not(embedded_ygg))]
fn embedded_bridge_retry_peers() -> Result<EmbeddedBridgeStatus> {
    Err(anyhow!("embedded Yggstack bridge is not compiled into this build"))
}

#[cfg(embedded_ygg)]
fn embedded_bridge_stop() -> Result<EmbeddedBridgeStatus> {
    unsafe { call_bridge(ffi::YggstackBridgeStop) }
}

#[cfg(not(embedded_ygg))]
fn embedded_bridge_stop() -> Result<EmbeddedBridgeStatus> {
    Err(anyhow!("embedded Yggstack bridge is not compiled into this build"))
}

#[cfg(embedded_ygg)]
unsafe fn call_bridge(function: unsafe extern "C" fn() -> *mut c_char) -> Result<EmbeddedBridgeStatus> {
    let ptr = function();
    if ptr.is_null() {
        return Err(anyhow!("embedded Yggstack bridge returned a null pointer"));
    }

    let raw = CStr::from_ptr(ptr).to_string_lossy().into_owned();
    ffi::YggstackBridgeFreeString(ptr);

    let status: EmbeddedBridgeStatus =
        serde_json::from_str(&raw).with_context(|| format!("invalid embedded Yggstack JSON: {raw}"))?;
    Ok(status)
}

#[cfg(embedded_ygg)]
mod ffi {
    use std::os::raw::c_char;

    unsafe extern "C" {
        pub fn YggstackBridgeStartAutoconf() -> *mut c_char;
        pub fn YggstackBridgeStatus() -> *mut c_char;
        pub fn YggstackBridgeRetryPeers() -> *mut c_char;
        pub fn YggstackBridgeStop() -> *mut c_char;
        pub fn YggstackBridgeFreeString(ptr: *mut c_char);
    }
}
