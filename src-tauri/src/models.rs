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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum TransportKind {
    #[default]
    Unknown,
    Direct,
    Relay,
    ReverseTunnel,
    MeshFallback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum LocalTargetState {
    #[default]
    Unknown,
    Reachable,
    Unreachable,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub peer_id: String,
    pub addr: String,
    pub connected: bool,
    pub ping_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct SwarmBootstrap {
    pub peer_id: String,
    pub listen_addrs: Vec<String>,
    pub relay_addrs: Vec<String>,
    pub nat_status: String,
    pub local_game_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatus {
    pub mode: SessionMode,
    pub state: ConnectionState,
    pub room_code: Option<String>,
    pub udp_bind_addr: Option<String>,
    pub public_udp_addr: Option<String>,
    pub local_game_port: Option<u16>,
    pub minecraft_version: Option<String>,
    pub transport_kind: TransportKind,
    pub local_target_state: LocalTargetState,
    pub transport_path: Option<String>,
    pub geyser_enabled: bool,
    pub bedrock_port: Option<u16>,
    pub password_protected: bool,
    pub peer_count: usize,
    pub peers: Vec<PeerInfo>,
    pub note: Option<String>,
    pub last_error: Option<String>,
    pub signaling_server: String,
    pub logs: Vec<String>,
}

impl Default for NetworkStatus {
    fn default() -> Self {
        Self {
            mode: SessionMode::Idle,
            state: ConnectionState::Idle,
            room_code: None,
            udp_bind_addr: None,
            public_udp_addr: None,
            local_game_port: None,
            minecraft_version: None,
            transport_kind: TransportKind::Unknown,
            local_target_state: LocalTargetState::Unknown,
            transport_path: None,
            geyser_enabled: false,
            bedrock_port: None,
            password_protected: false,
            peer_count: 0,
            peers: Vec::new(),
            note: None,
            last_error: None,
            signaling_server: String::new(),
            logs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PreflightReport {
    pub local_port: u16,
    pub reachable: bool,
    pub state: LocalTargetState,
    pub minecraft_version: Option<String>,
    pub recommended_host_action: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LanPortDetection {
    pub port: u16,
    pub source_path: String,
    pub source_line: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MinecraftNicknameDetection {
    pub nickname: String,
    pub source_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TestServerInfo {
    pub bind_addr: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSnapshot {
    pub exported_at: String,
    pub role: SessionMode,
    pub status: NetworkStatus,
    pub preflight: Option<PreflightReport>,
    pub test_server: Option<TestServerInfo>,
    pub geyser: Option<GeyserRuntimeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct GeyserRuntimeInfo {
    pub enabled: bool,
    pub running: bool,
    pub java_path: Option<String>,
    pub jar_path: Option<String>,
    pub runtime_dir: Option<String>,
    pub config_path: Option<String>,
    pub log_path: Option<String>,
    pub bedrock_port: Option<u16>,
    pub bedrock_public_endpoint: Option<String>,
    pub firewall_rule_name: Option<String>,
    pub note: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub product_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub available: bool,
    pub release_url: Option<String>,
    pub download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallUpdateResult {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExternalServerProbe {
    pub room_name: String,
    pub host_name: String,
    pub version: Option<String>,
    pub online_players: u32,
    pub max_players: u32,
    pub ping_ms: Option<u64>,
}
