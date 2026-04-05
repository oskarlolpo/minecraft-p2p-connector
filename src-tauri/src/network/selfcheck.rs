use std::{net::SocketAddr, sync::Arc, time::Duration};

use anyhow::{Context, Result};
use hickory_resolver::{
    config::{ResolverConfig, ResolverOpts, GOOGLE},
    net::runtime::TokioRuntimeProvider,
    Resolver,
};
use tokio::{net::TcpStream, net::UdpSocket, time::timeout};

use crate::{
    models::{CheckResult, NetworkChecks},
    signaling::{discover_public_addr, SignalingConfig},
};

const ABLY_MQTT_ADDR: &str = "main.mqtt.ably.net:8883";
pub async fn run_network_self_check(cloudflare_endpoint: Option<&str>) -> NetworkChecks {
    let ably_tcp = check_tcp(ABLY_MQTT_ADDR).await;
    let system_dns = check_system_dns("main.mqtt.ably.net").await;
    let fallback_dns = check_fallback_dns("main.mqtt.ably.net").await;
    let cloudflare_https = match cloudflare_endpoint {
        Some(endpoint) if !endpoint.trim().is_empty() => Some(check_https(endpoint).await),
        _ => None,
    };
    let turn_udp = check_turn_udp().await;

    NetworkChecks {
        ably_tcp,
        system_dns,
        fallback_dns,
        cloudflare_https,
        turn_udp,
    }
}

async fn check_tcp(target: &str) -> CheckResult {
    match timeout(Duration::from_secs(3), TcpStream::connect(target)).await {
        Ok(Ok(stream)) => {
            let peer = stream
                .peer_addr()
                .map(|value| value.to_string())
                .unwrap_or_else(|_| target.to_string());
            CheckResult {
                ok: true,
                detail: format!("TCP reachability OK via {peer}"),
            }
        }
        Ok(Err(error)) => CheckResult {
            ok: false,
            detail: format!("TCP connect failed: {error}"),
        },
        Err(_) => CheckResult {
            ok: false,
            detail: "TCP connect timed out".into(),
        },
    }
}

async fn check_system_dns(host: &str) -> CheckResult {
    match tokio::net::lookup_host((host, 443)).await {
        Ok(mut records) => {
            let value = records
                .next()
                .map(|entry| entry.ip().to_string())
                .unwrap_or_else(|| "no records".into());
            CheckResult {
                ok: true,
                detail: format!("System DNS resolved {host} -> {value}"),
            }
        }
        Err(error) => CheckResult {
            ok: false,
            detail: format!("System DNS failed: {error}"),
        },
    }
}

async fn check_fallback_dns(host: &str) -> CheckResult {
    let resolver = Resolver::builder_with_config(
        ResolverConfig::udp_and_tcp(&GOOGLE),
        TokioRuntimeProvider::default(),
    )
    .with_options(ResolverOpts::default())
    .build();

    let resolver = match resolver {
        Ok(resolver) => resolver,
        Err(error) => {
            return CheckResult {
                ok: false,
                detail: format!("Fallback DNS resolver init failed: {error}"),
            };
        }
    };

    match resolver.lookup_ip(host).await {
        Ok(records) => {
            let value = records
                .iter()
                .next()
                .map(|entry: std::net::IpAddr| entry.to_string())
                .unwrap_or_else(|| "no records".into());
            CheckResult {
                ok: true,
                detail: format!("Fallback DNS resolved {host} -> {value}"),
            }
        }
        Err(error) => CheckResult {
            ok: false,
            detail: format!("Fallback DNS failed: {error}"),
        },
    }
}

async fn check_https(url: &str) -> CheckResult {
    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .build()
    {
        Ok(client) => client,
        Err(error) => {
            return CheckResult {
                ok: false,
                detail: format!("HTTP client build failed: {error}"),
            };
        }
    };

    match client.get(url).send().await {
        Ok(response) => CheckResult {
            ok: response.status().is_success(),
            detail: format!("HTTPS {} -> {}", url, response.status()),
        },
        Err(error) => CheckResult {
            ok: false,
            detail: format!("HTTPS request failed: {error}"),
        },
    }
}

async fn check_turn_udp() -> CheckResult {
    match probe_turn_udp().await {
        Ok(public_addr) => CheckResult {
            ok: true,
            detail: format!("TURN/STUN UDP path reachable, mapped address {public_addr}"),
        },
        Err(error) => CheckResult {
            ok: false,
            detail: format!("TURN/STUN UDP precheck failed: {error:#}"),
        },
    }
}

async fn probe_turn_udp() -> Result<SocketAddr> {
    let socket = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
    let config = SignalingConfig {
        stun_servers: vec!["stun.cloudflare.com:3478".into()],
    };

    discover_public_addr(socket, &config)
        .await
        .context("Cloudflare STUN check failed")
}
