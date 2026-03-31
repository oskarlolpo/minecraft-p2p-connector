use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum SessionMode {
    #[default]
    Idle,
    Host,
    Client,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ConnectionState {
    #[default]
    Idle,
    Starting,
    WaitingForPeer,
    Punching,
    Connecting,
    Hosting,
    Connected,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub peer_id: String,
    pub addr: String,
    pub connected: bool,
    pub ping_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub mode: SessionMode,
    pub state: ConnectionState,
    pub room_code: Option<String>,
    pub udp_bind_addr: Option<String>,
    pub public_udp_addr: Option<String>,
    pub peer_count: usize,
    pub peers: Vec<PeerInfo>,
    pub note: Option<String>,
    pub last_error: Option<String>,
    pub signaling_server: String,
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self {
            mode: SessionMode::Idle,
            state: ConnectionState::Idle,
            room_code: None,
            udp_bind_addr: None,
            public_udp_addr: None,
            peer_count: 0,
            peers: Vec::new(),
            note: None,
            last_error: None,
            signaling_server: String::new(),
        }
    }
}
