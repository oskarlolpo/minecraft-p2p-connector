use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_TURN_URLS: &str = concat!(
    "stun:stun.cloudflare.com:3478,",
    "turn:turn.cloudflare.com:3478?transport=udp,",
    "turn:turn.cloudflare.com:3478?transport=tcp,",
    "turns:turn.cloudflare.com:5349?transport=tcp,",
    "turns:turn.cloudflare.com:443?transport=tcp"
);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareConfig {
    pub credential_endpoint: Option<String>,
    pub turn_urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareIceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CloudflareIceServerResponse {
    pub ice_servers: Vec<CloudflareIceServer>,
    pub ttl: Option<u64>,
    pub note: Option<String>,
}

impl CloudflareConfig {
    pub fn from_env() -> Self {
        let credential_endpoint = std::env::var("MC_CF_TURN_CREDENTIAL_ENDPOINT")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        let turn_urls = std::env::var("MC_CF_TURN_URLS")
            .unwrap_or_else(|_| DEFAULT_TURN_URLS.into())
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        Self {
            credential_endpoint,
            turn_urls,
        }
    }

    pub fn runtime_available(&self) -> bool {
        self.credential_endpoint.is_some()
    }

    pub fn first_turn_endpoint(&self) -> Option<String> {
        self.turn_urls
            .iter()
            .find(|value| value.starts_with("turn:") || value.starts_with("turns:"))
            .cloned()
    }

    pub async fn probe_runtime(&self) -> Result<()> {
        let endpoint = self
            .credential_endpoint
            .as_ref()
            .context("MC_CF_TURN_CREDENTIAL_ENDPOINT is not configured")?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(8))
            .build()
            .context("failed to build Cloudflare HTTP client")?;
        let response = client
            .get(endpoint)
            .send()
            .await
            .with_context(|| format!("failed to reach Cloudflare credential endpoint {endpoint}"))?;
        if !response.status().is_success() {
            anyhow::bail!("Cloudflare credential endpoint returned {}", response.status());
        }
        Ok(())
    }

    pub async fn fetch_ice_servers(&self) -> Result<CloudflareIceServerResponse> {
        let endpoint = self
            .credential_endpoint
            .as_ref()
            .context("MC_CF_TURN_CREDENTIAL_ENDPOINT is not configured")?;
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(12))
            .build()
            .context("failed to build Cloudflare HTTP client")?;
        let response = client
            .get(endpoint)
            .send()
            .await
            .with_context(|| format!("failed to fetch ICE servers from {endpoint}"))?;
        let status = response.status();
        if !status.is_success() {
            anyhow::bail!("Cloudflare credential endpoint returned {status}");
        }

        let mut payload = response
            .json::<CloudflareIceServerResponse>()
            .await
            .context("failed to decode Cloudflare ICE server response")?;
        if payload.ice_servers.is_empty() {
            payload.ice_servers = default_ice_servers(&self.turn_urls);
        }
        Ok(payload)
    }
}

fn default_ice_servers(urls: &[String]) -> Vec<CloudflareIceServer> {
    vec![CloudflareIceServer {
        urls: urls.to_vec(),
        username: None,
        credential: None,
    }]
}
