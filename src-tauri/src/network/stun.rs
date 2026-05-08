use std::{net::SocketAddr, time::Duration};

use anyhow::{anyhow, Context, Result};
use tokio::net::UdpSocket;

/// STUN Binding Request (RFC 5389)
/// Magic cookie: 0x2112A442
/// Transaction ID: 12 random bytes
const STUN_MAGIC: u32 = 0x2112_A442;
const STUN_BINDING_REQUEST: u16 = 0x0001;
const STUN_BINDING_RESPONSE: u16 = 0x0101;
const STUN_ATTR_XOR_MAPPED: u16 = 0x0020;
const STUN_ATTR_MAPPED: u16 = 0x0001;

const STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302",
    "stun.cloudflare.com:3478",
];

const STUN_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NatTypeResult {
    pub nat_type: String,
    pub public_ip: Option<String>,
    pub public_port: Option<u16>,
    pub mapped_addresses: Vec<String>,
    pub note: String,
}

/// Выполняет STUN Binding Request и возвращает публичный адрес.
async fn stun_binding_request(socket: &UdpSocket, server: &str) -> Result<SocketAddr> {
    let resolved: Vec<SocketAddr> = tokio::net::lookup_host(server)
        .await
        .context("DNS lookup STUN server failed")?
        .collect();
    let server_addr = resolved
        .first()
        .ok_or_else(|| anyhow!("STUN server {server} не резолвится"))?;

    // Build STUN Binding Request
    let tx_id: [u8; 12] = rand_tx_id();
    let mut packet = Vec::with_capacity(20);
    packet.extend_from_slice(&STUN_BINDING_REQUEST.to_be_bytes()); // Type
    packet.extend_from_slice(&0u16.to_be_bytes()); // Length (no attributes)
    packet.extend_from_slice(&STUN_MAGIC.to_be_bytes()); // Magic Cookie
    packet.extend_from_slice(&tx_id); // Transaction ID

    socket.send_to(&packet, server_addr).await?;

    let mut buf = [0u8; 512];
    let (len, _from) = tokio::time::timeout(STUN_TIMEOUT, socket.recv_from(&mut buf))
        .await
        .context("STUN timeout")?
        .context("STUN recv failed")?;

    if len < 20 {
        return Err(anyhow!("STUN response too short"));
    }

    let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
    if msg_type != STUN_BINDING_RESPONSE {
        return Err(anyhow!("unexpected STUN message type: 0x{msg_type:04x}"));
    }

    // Verify transaction ID
    if buf[8..20] != tx_id {
        return Err(anyhow!("STUN transaction ID mismatch"));
    }

    let attrs_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    let attrs_end = std::cmp::min(20 + attrs_len, len);

    // Parse attributes
    let mut offset = 20;
    while offset + 4 <= attrs_end {
        let attr_type = u16::from_be_bytes([buf[offset], buf[offset + 1]]);
        let attr_len = u16::from_be_bytes([buf[offset + 2], buf[offset + 3]]) as usize;
        let attr_start = offset + 4;
        let attr_end = attr_start + attr_len;
        if attr_end > attrs_end {
            break;
        }

        if attr_type == STUN_ATTR_XOR_MAPPED && attr_len >= 8 {
            let family = buf[attr_start + 1];
            if family == 0x01 {
                // IPv4
                let xor_port =
                    u16::from_be_bytes([buf[attr_start + 2], buf[attr_start + 3]]) ^ (STUN_MAGIC >> 16) as u16;
                let xor_ip = u32::from_be_bytes([
                    buf[attr_start + 4],
                    buf[attr_start + 5],
                    buf[attr_start + 6],
                    buf[attr_start + 7],
                ]) ^ STUN_MAGIC;
                let ip = std::net::Ipv4Addr::from(xor_ip);
                return Ok(SocketAddr::new(ip.into(), xor_port));
            }
        }

        if attr_type == STUN_ATTR_MAPPED && attr_len >= 8 {
            let family = buf[attr_start + 1];
            if family == 0x01 {
                let port = u16::from_be_bytes([buf[attr_start + 2], buf[attr_start + 3]]);
                let ip = std::net::Ipv4Addr::new(
                    buf[attr_start + 4],
                    buf[attr_start + 5],
                    buf[attr_start + 6],
                    buf[attr_start + 7],
                );
                return Ok(SocketAddr::new(ip.into(), port));
            }
        }

        // Align to 4 bytes
        offset = attr_end + (4 - (attr_len % 4)) % 4;
    }

    Err(anyhow!("no MAPPED-ADDRESS in STUN response"))
}

fn rand_tx_id() -> [u8; 12] {
    let mut id = [0u8; 12];
    let uuid_bytes = uuid::Uuid::new_v4();
    id.copy_from_slice(&uuid_bytes.as_bytes()[..12]);
    id
}

/// Определяет тип NAT используя два STUN сервера.
/// - Если оба возвращают один и тот же IP:port → Full Cone / Open
/// - Если IP одинаковый, но порты разные → Symmetric NAT  
/// - Если один из запросов фейлит → Restricted / Firewall
pub async fn detect_nat_type() -> NatTypeResult {
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(e) => {
            return NatTypeResult {
                nat_type: "error".into(),
                note: format!("Failed to bind UDP socket: {e}"),
                ..Default::default()
            }
        }
    };

    let mut mapped: Vec<SocketAddr> = Vec::new();
    let mut errors: Vec<String> = Vec::new();

    for server in STUN_SERVERS.iter().take(2) {
        match stun_binding_request(&socket, server).await {
            Ok(addr) => mapped.push(addr),
            Err(e) => errors.push(format!("{server}: {e}")),
        }
    }

    if mapped.is_empty() {
        return NatTypeResult {
            nat_type: "blocked".into(),
            note: format!(
                "Все STUN серверы недоступны. UDP скорее всего заблокирован. Ошибки: {}",
                errors.join("; ")
            ),
            ..Default::default()
        };
    }

    let public_ip = Some(mapped[0].ip().to_string());
    let public_port = Some(mapped[0].port());
    let mapped_strs: Vec<String> = mapped.iter().map(|a| a.to_string()).collect();

    if mapped.len() == 1 {
        return NatTypeResult {
            nat_type: "restricted".into(),
            public_ip,
            public_port,
            mapped_addresses: mapped_strs,
            note: format!(
                "Один STUN ответил ({}), второй недоступен. NAT скорее всего Restricted или Port-Restricted. P2P возможен с hole-punching.",
                mapped[0]
            ),
        };
    }

    // Two responses
    let same_ip = mapped[0].ip() == mapped[1].ip();
    let same_port = mapped[0].port() == mapped[1].port();

    let nat_type = if same_ip && same_port {
        "open" // Full Cone or No NAT
    } else if same_ip && !same_port {
        "symmetric" // Different ports → Symmetric NAT
    } else {
        "multiple_ips" // Different IPs — very unusual
    };

    let note = match nat_type {
        "open" => format!(
            "NAT тип: Open / Full Cone. Публичный адрес: {}. P2P работает отлично.",
            mapped[0]
        ),
        "symmetric" => format!(
            "NAT тип: Symmetric. Порт меняется для каждого соединения ({} vs {}). P2P QUIC hole-punching может не работать. Рекомендуется использовать e4mc или WSS relay.",
            mapped[0], mapped[1]
        ),
        "multiple_ips" => format!(
            "Обнаружено несколько публичных IP ({} и {}). Возможно используется балансировщик нагрузки или многоуровневый NAT.",
            mapped[0], mapped[1]
        ),
        _ => String::new(),
    };

    NatTypeResult {
        nat_type: nat_type.into(),
        public_ip,
        public_port,
        mapped_addresses: mapped_strs,
        note,
    }
}

/// Preflight: Проверяет, свободен ли локальный TCP-порт (bind-test).
/// Возвращает Ok(()) если свободен, Err с описанием если занят.
pub fn preflight_port_check(port: u16) -> Result<()> {
    use std::net::TcpListener;
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse()?;
    match TcpListener::bind(addr) {
        Ok(listener) => {
            drop(listener);
            Ok(())
        }
        Err(e) => Err(anyhow!(
            "Порт {port} на 127.0.0.1 уже занят другим процессом: {e}. \
             Закройте приложение, использующее этот порт, или выберите другой."
        )),
    }
}
