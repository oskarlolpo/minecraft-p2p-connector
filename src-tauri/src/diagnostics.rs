use std::sync::Arc;

use tokio::sync::Mutex;

use crate::models::{CloudflareAttempt, NetworkChecks, TransportAttempt};

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsStore {
    inner: Arc<Mutex<DiagnosticsState>>,
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticsState {
    pub network_checks: Option<NetworkChecks>,
    pub direct_attempt: Option<TransportAttempt>,
    pub cloudflare_attempt: Option<CloudflareAttempt>,
    pub selected_transport: Option<String>,
}

impl DiagnosticsStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn snapshot(&self) -> DiagnosticsState {
        self.inner.lock().await.clone()
    }

    #[allow(dead_code)]
    pub async fn clear(&self) {
        *self.inner.lock().await = DiagnosticsState::default();
    }

    pub async fn set_network_checks(&self, checks: NetworkChecks) {
        self.inner.lock().await.network_checks = Some(checks);
    }

    pub async fn set_direct_attempt(&self, attempt: TransportAttempt) {
        self.inner.lock().await.direct_attempt = Some(attempt);
    }

    pub async fn set_cloudflare_attempt(&self, attempt: CloudflareAttempt) {
        self.inner.lock().await.cloudflare_attempt = Some(attempt);
    }

    pub async fn set_selected_transport(&self, transport: impl Into<String>) {
        self.inner.lock().await.selected_transport = Some(transport.into());
    }
}
