use serde::Serialize;

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
}
