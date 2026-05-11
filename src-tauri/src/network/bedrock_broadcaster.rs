use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};
use anyhow::Result;

const RAKNET_MAGIC: [u8; 16] = [
    0x00, 0xff, 0xff, 0x00, 0xfe, 0xfe, 0xfe, 0xfe, 0xfd, 0xfd, 0xfd, 0xfd, 0x12, 0x34, 0x56, 0x78,
];

pub struct BedrockBroadcaster {
    task: tokio::task::JoinHandle<()>,
}

impl BedrockBroadcaster {
    pub async fn start(
        room_name: String,
        proxy_port: u16,
        cancel: CancellationToken,
    ) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:19132").await;
        let socket = match socket {
            Ok(s) => s,
            Err(_) => {
                warn!("UDP port 19132 is occupied. Binding to random port for Bedrock Broadcaster.");
                UdpSocket::bind("0.0.0.0:0").await?
            }
        };

        let local_addr = socket.local_addr()?;
        info!("Bedrock LAN Broadcaster started on {}", local_addr);

        let task = tokio::spawn(async move {
            let mut buf = [0u8; 1024];
            loop {
                tokio::select! {
                    res = socket.recv_from(&mut buf) => {
                        match res {
                            Ok((len, addr)) => {
                                if len > 17 && (buf[0] == 0x01 || buf[0] == 0x02) {
                                    // RakNet Unconnected Ping
                                    let ping_time = &buf[1..9];
                                    
                                    let mut pong = Vec::with_capacity(256);
                                    pong.push(0x1c); // Unconnected Pong
                                    pong.extend_from_slice(ping_time);
                                    
                                    // Server GUID (mocked)
                                    let server_guid: u64 = 1234567890;
                                    pong.extend_from_slice(&server_guid.to_be_bytes());
                                    pong.extend_from_slice(&RAKNET_MAGIC);
                                    
                                    // MOTD
                                    // MCPE;Server Name;ProtocolVersion;VersionString;Online;Max;ServerUID;SecondLine;GameMode;GameModeNumeric;Portv4;Portv6;
                                    let motd = format!(
                                        "MCPE;{};776;1.21.51;0;30;1234567890;P2P Connector;Survival;1;{};19133;",
                                        room_name, proxy_port
                                    );
                                    
                                    pong.extend_from_slice(&(motd.len() as u16).to_be_bytes());
                                    pong.extend_from_slice(motd.as_bytes());
                                    
                                    if let Err(e) = socket.send_to(&pong, addr).await {
                                        debug!("Failed to send RakNet pong to {}: {}", addr, e);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Bedrock broadcaster socket error: {}", e);
                            }
                        }
                    }
                    _ = cancel.cancelled() => {
                        info!("Shutting down Bedrock LAN Broadcaster.");
                        break;
                    }
                }
            }
        });

        Ok(Self { task })
    }

    pub async fn stop(self) {
        self.task.abort();
    }
}

