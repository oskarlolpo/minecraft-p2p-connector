use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use tauri::{AppHandle, Emitter};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, RwLock},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use crate::models::{NetworkStatus, TestServerInfo};

struct TestServerRuntime {
    cancel: CancellationToken,
    task: JoinHandle<()>,
    info: TestServerInfo,
}

#[derive(Clone, Default)]
pub struct TestServerManager {
    runtime: Arc<Mutex<Option<TestServerRuntime>>>,
}

impl TestServerManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn start(
        &self,
        app: AppHandle,
        status: Arc<RwLock<NetworkStatus>>,
        port: u16,
    ) -> Result<TestServerInfo> {
        self.stop(status.clone()).await?;

        let listener = TcpListener::bind(("127.0.0.1", port))
            .await
            .with_context(|| format!("не удалось запустить тестовый сервер на 127.0.0.1:{port}"))?;
        let info = TestServerInfo {
            bind_addr: listener.local_addr()?.to_string(),
            protocol: "echo-timestamp".into(),
        };

        let cancel = CancellationToken::new();
        let loop_cancel = cancel.clone();
        let info_for_task = info.clone();
        let task = tokio::spawn(async move {
            log_status(
                &status,
                format!(
                    "Тестовый сервер запущен на {}. Протокол: {}.",
                    info_for_task.bind_addr, info_for_task.protocol
                ),
            )
            .await;
            let _ = app.emit("test_server_started", &info_for_task);

            loop {
                tokio::select! {
                    _ = loop_cancel.cancelled() => break,
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok((stream, addr)) => {
                                let status = status.clone();
                                let app = app.clone();
                                tokio::spawn(async move {
                                    if let Err(error) = handle_client(stream, addr.to_string(), status.clone()).await {
                                        log_status(&status, format!("Тестовый сервер: ошибка клиента {addr}: {error:#}")).await;
                                    }
                                    let _ = app.emit("test_server_client_closed", addr.to_string());
                                });
                            }
                            Err(error) => {
                                log_status(&status, format!("Тестовый сервер: accept() вернул ошибку: {error}")).await;
                            }
                        }
                    }
                }
            }

            log_status(&status, "Тестовый сервер остановлен.".into()).await;
        });

        *self.runtime.lock().await = Some(TestServerRuntime {
            cancel,
            task,
            info: info.clone(),
        });

        Ok(info)
    }

    pub async fn stop(&self, status: Arc<RwLock<NetworkStatus>>) -> Result<()> {
        if let Some(runtime) = self.runtime.lock().await.take() {
            runtime.cancel.cancel();
            let _ = runtime.task.await;
            log_status(
                &status,
                format!("Тестовый сервер {} остановлен вручную.", runtime.info.bind_addr),
            )
            .await;
        }
        Ok(())
    }

    pub async fn current_info(&self) -> Option<TestServerInfo> {
        self.runtime
            .lock()
            .await
            .as_ref()
            .map(|runtime| runtime.info.clone())
    }
}

pub async fn probe_test_server(port: u16, payload: String) -> Result<String> {
    let target = format!("127.0.0.1:{port}");
    let mut stream = TcpStream::connect(&target)
        .await
        .with_context(|| format!("не удалось подключиться к тестовому серверу {target}"))?;
    let message = format!("{payload}\n");
    stream
        .write_all(message.as_bytes())
        .await
        .with_context(|| format!("не удалось отправить тестовый пакет на {target}"))?;
    stream
        .flush()
        .await
        .with_context(|| format!("не удалось flush для {target}"))?;

    let mut buffer = vec![0u8; 2048];
    let read = stream
        .read(&mut buffer)
        .await
        .with_context(|| format!("не удалось прочитать ответ от {target}"))?;
    if read == 0 {
        return Ok(String::new());
    }

    Ok(String::from_utf8_lossy(&buffer[..read]).trim().to_string())
}

async fn handle_client(
    mut stream: TcpStream,
    remote_addr: String,
    status: Arc<RwLock<NetworkStatus>>,
) -> Result<()> {
    log_status(
        &status,
        format!("Тестовый сервер: клиент {remote_addr} подключился."),
    )
    .await;

    let mut buf = vec![0u8; 4096];
    loop {
        let read = stream.read(&mut buf).await?;
        if read == 0 {
            log_status(
                &status,
                format!("Тестовый сервер: клиент {remote_addr} закрыл соединение."),
            )
            .await;
            break;
        }

        let received = String::from_utf8_lossy(&buf[..read]).trim().to_string();
        log_status(
            &status,
            format!("Тестовый сервер: принято {read} байт от {remote_addr}: {received}"),
        )
        .await;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or_default();
        let response = format!("echo:{timestamp}:{received}\n");
        stream.write_all(response.as_bytes()).await?;
        stream.flush().await?;
    }

    Ok(())
}

async fn log_status(status: &Arc<RwLock<NetworkStatus>>, message: String) {
    let mut guard = status.write().await;
    guard.logs.push(message);
    if guard.logs.len() > 240 {
        let overflow = guard.logs.len() - 240;
        guard.logs.drain(0..overflow);
    }
}
