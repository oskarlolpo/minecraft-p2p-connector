use std::{collections::HashSet, pin::Pin, str::FromStr, sync::Arc, time::Duration};

use anyhow::{anyhow, Context, Result};
use futures::StreamExt;
use libp2p::{
    autonat,
    core::{multiaddr::Protocol, ConnectedPoint, Multiaddr},
    dns,
    dcutr, identify, noise, relay,
    swarm::{
        dial_opts::{DialOpts, PeerCondition},
        NetworkBehaviour, SwarmEvent,
    },
    tcp, tls, yamux, PeerId, Stream, StreamProtocol, Swarm, SwarmBuilder,
};
use libp2p_stream::{Control as StreamControl, IncomingStreams};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{mpsc, oneshot, Mutex, RwLock},
    task::JoinHandle,
    time::timeout,
};
use tokio_util::{compat::FuturesAsyncReadCompatExt, sync::CancellationToken};

use crate::models::{
    ConnectionState, LocalTargetState, NetworkStatus, PeerInfo, SessionMode, SwarmBootstrap,
    TransportKind,
};

use super::{
    minecraft::detect_local_version,
    tunnel::{self, ReverseTunnelConfig, ReverseTunnelEndpoint, ReverseTunnelHandle, DEFAULT_BORE_HOST},
};

const APP_PROTOCOL: &str = "/blood-paradise-hub/2.0.0";
const MINECRAFT_STREAM_PROTOCOL: &str = "/mc-p2p/1.0.0";
const CLIENT_LOCAL_BIND_ADDR: &str = "127.0.0.1:25565";
const RELAY_BOOTSTRAPS_ENV: &str = "MC_LIBP2P_RELAYS";
const SIGNALING_LABEL: &str = "Ably Presence (PeerId + Multiaddr)";
const MAX_LOG_LINES: usize = 240;
const DEFAULT_RELAY_BOOTSTRAPS: &[&str] = &[
    "/dns4/sv15.bootstrap.libp2p.io/tcp/443/wss/p2p/QmNnooDu7bfjPFoTZYxMNLWUQJyrVwtbZg5gBMjTezGAJN",
    "/dns4/ams-1.bootstrap.libp2p.io/tcp/443/wss/p2p/QmbLHAnMoJPWSCR5Zhtx6BHJX9KiKNN6tpvbUcqanj75Nb",
];
const BOOTSTRAP_READY_TIMEOUT_SECS: u64 = 18;
const REVERSE_TUNNEL_FALLBACK_SECS: u64 = 15;

#[derive(NetworkBehaviour)]
#[behaviour(prelude = "libp2p_swarm::derive_prelude", to_swarm = "ConnectorEvent")]
struct ConnectorBehaviour {
    relay: relay::client::Behaviour,
    dcutr: dcutr::Behaviour,
    identify: identify::Behaviour,
    autonat: autonat::Behaviour,
    stream: libp2p_stream::Behaviour,
}

#[derive(Debug)]
enum ConnectorEvent {
    Relay(relay::client::Event),
    Dcutr(dcutr::Event),
    Identify(identify::Event),
    Autonat(autonat::Event),
    Stream(()),
}

impl From<relay::client::Event> for ConnectorEvent {
    fn from(event: relay::client::Event) -> Self {
        Self::Relay(event)
    }
}

impl From<dcutr::Event> for ConnectorEvent {
    fn from(event: dcutr::Event) -> Self {
        Self::Dcutr(event)
    }
}

impl From<identify::Event> for ConnectorEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

impl From<autonat::Event> for ConnectorEvent {
    fn from(event: autonat::Event) -> Self {
        Self::Autonat(event)
    }
}

impl From<()> for ConnectorEvent {
    fn from(_: ()) -> Self {
        Self::Stream(())
    }
}

#[derive(Clone)]
pub struct NetworkSwarmManager {
    status: Arc<RwLock<NetworkStatus>>,
    runtime: Arc<Mutex<Option<SwarmRuntime>>>,
}

struct SwarmRuntime {
    mode: SessionMode,
    active_route: Arc<RwLock<Option<ActiveRoute>>>,
    command_tx: mpsc::Sender<SwarmCommand>,
    cancel: CancellationToken,
    handles: Vec<JoinHandle<()>>,
}

enum SwarmCommand {
    DialPeer { peer_id: PeerId, addrs: Vec<Multiaddr> },
    KickPeer { peer_id: PeerId },
}

#[derive(Debug, Clone)]
enum ActiveRoute {
    Libp2pPeer(PeerId),
    ReverseTunnel(ReverseTunnelEndpoint),
}

#[derive(Clone)]
struct SwarmSessionConfig {
    mode: SessionMode,
    room_name: Option<String>,
    password_protected: bool,
    host_local_port: Option<u16>,
    minecraft_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PeerConnectedPayload {
    peer_id: String,
    addr: String,
    relayed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct RelayActivePayload {
    relay_addr: String,
}

#[derive(Debug, Clone, Serialize)]
struct HolePunchPayload {
    peer_id: String,
}

#[derive(Debug, Clone, Serialize)]
struct ReverseTunnelReadyPayload {
    endpoint: String,
    multiaddr: String,
}

#[derive(Debug, Clone, Serialize)]
struct ConnectionSuccessPayload {
    addr: String,
    transport: String,
}

impl NetworkSwarmManager {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(default_status())),
            runtime: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn get_status(&self) -> NetworkStatus {
        self.status.read().await.clone()
    }

    pub fn shared_status(&self) -> Arc<RwLock<NetworkStatus>> {
        self.status.clone()
    }

    pub async fn start_hosting(
        &self,
        app: AppHandle,
        room_name: String,
        password: Option<String>,
        local_port: u16,
    ) -> Result<SwarmBootstrap> {
        let mut runtime_guard = self.runtime.lock().await;
        if runtime_guard.is_some() {
            return Err(anyhow!("libp2p-СЃРµСЃСЃРёСЏ СѓР¶Рµ Р°РєС‚РёРІРЅР°"));
        }

        let minecraft_version = detect_local_version(local_port).await.ok();
        let password_protected = password
            .as_ref()
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let local_target_ready = verify_local_minecraft_target(local_port).await.is_ok();

        {
            let mut status = self.status.write().await;
            *status = default_status();
            status.mode = SessionMode::Host;
            status.state = ConnectionState::Starting;
            status.room_code = Some(room_name.clone());
            status.local_game_port = Some(local_port);
            status.minecraft_version = minecraft_version.clone();
            status.local_target_state = if local_target_ready {
                LocalTargetState::Reachable
            } else {
                LocalTargetState::Unreachable
            };
            status.password_protected = password_protected;
            status.note = Some(
                "Запускаем сетевую сессию, проверяем локальный Minecraft и подготавливаем direct/fallback транспорт."
                    .into(),
            );
            push_log(
                &mut status,
                format!("Старт хоста \"{room_name}\" через networking core нового поколения."),
            );
            if !local_target_ready {
                push_log(
                    &mut status,
                    format!(
                        "Предупреждение: локальный Minecraft на 127.0.0.1:{local_port} недоступен. Откройте мир в LAN или запустите сервер перед подключением игроков."
                    ),
                );
            }
        }

        let (runtime, bootstrap) = spawn_swarm_runtime(
            app,
            self.status.clone(),
            SwarmSessionConfig {
                mode: SessionMode::Host,
                room_name: Some(room_name),
                password_protected,
                host_local_port: Some(local_port),
                minecraft_version,
            },
        )
        .await?;

        *runtime_guard = Some(runtime);
        Ok(bootstrap)
    }

    pub async fn stop_hosting(&self) -> Result<()> {
        let runtime = self.runtime.lock().await.take();

        if let Some(runtime) = runtime {
            runtime.cancel.cancel();
            for handle in runtime.handles {
                let _ = handle.await;
            }
        }

        let mut status = self.status.write().await;
        *status = default_status();
        push_log(&mut status, "Сетевая сессия остановлена.");
        Ok(())
    }

    pub async fn connect_to_peer(
        &self,
        app: AppHandle,
        peer_id: String,
        peer_addrs: Vec<String>,
    ) -> Result<()> {
        let peer_id = PeerId::from_str(&peer_id).context("РЅРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ PeerId")?;
        let addrs = peer_addrs
            .into_iter()
            .map(|value| {
                normalize_multiaddr_input(&value)
                    .with_context(|| format!("РЅРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ multiaddr: {value}"))
            })
            .collect::<Result<Vec<_>>>()?;

        if addrs.is_empty() {
            return Err(anyhow!("РґР»СЏ РїРѕРґРєР»СЋС‡РµРЅРёСЏ РЅСѓР¶РµРЅ С…РѕС‚СЏ Р±С‹ РѕРґРёРЅ multiaddr"));
        }

        let mut runtime_guard = self.runtime.lock().await;
        if runtime_guard.is_none() {
            {
                let mut status = self.status.write().await;
                *status = default_status();
                status.mode = SessionMode::Client;
                status.state = ConnectionState::Starting;
                status.local_game_port = Some(25565);
                status.note = Some("Запускаем клиентскую сетевую сессию и выбираем лучший транспорт.".into());
                push_log(&mut status, "Инициализация клиентского networking core.");
            }

            let (runtime, _) = spawn_swarm_runtime(
                app.clone(),
                self.status.clone(),
                SwarmSessionConfig {
                    mode: SessionMode::Client,
                    room_name: None,
                    password_protected: false,
                    host_local_port: None,
                    minecraft_version: None,
                },
            )
            .await?;
            *runtime_guard = Some(runtime);
        }

        let runtime = runtime_guard
            .as_ref()
            .context("runtime РЅРµ Р±С‹Р» СЃРѕР·РґР°РЅ")?;

        if runtime.mode == SessionMode::Host {
            return Err(anyhow!(
                "РЅРµР»СЊР·СЏ dial'РёС‚СЊ СѓРґР°Р»С‘РЅРЅС‹Р№ peer, РїРѕРєР° Р·Р°РїСѓС‰РµРЅ Р»РѕРєР°Р»СЊРЅС‹Р№ С…РѕСЃС‚"
            ));
        }

        let reverse_target = reverse_tunnel_target_from_addrs(&addrs);
        {
            let mut active_route = runtime.active_route.write().await;
            *active_route = Some(match reverse_target.clone() {
                Some(endpoint) => ActiveRoute::ReverseTunnel(endpoint),
                None => ActiveRoute::Libp2pPeer(peer_id),
            });
        }

        {
            let mut status = self.status.write().await;
            status.mode = SessionMode::Client;
            status.state = ConnectionState::Connecting;
            status.transport_kind = TransportKind::Unknown;
            status.transport_path = None;
            status.note = Some("Поднимаем соединение с peer и пытаемся выбрать самый быстрый рабочий путь.".into());
            push_log(
                &mut status,
                format!(
                    "Подключение к peer {} по {} адресу(ам).",
                    peer_id,
                    addrs.len()
                ),
            );
        }

        if let Some(endpoint) = reverse_target {
            probe_reverse_tunnel_endpoint(&endpoint)
                .await
                .with_context(|| format!("не удалось выполнить TCP handshake к {}", endpoint.as_socket_label()))?;

            let mut status = self.status.write().await;
            status.state = ConnectionState::Connected;
            status.transport_kind = TransportKind::ReverseTunnel;
            status.transport_path = Some("reverse-tunnel".into());
            status.note = Some(format!(
                "Reverse tunnel активирован. Подключайтесь в Minecraft к {}.",
                CLIENT_LOCAL_BIND_ADDR
            ));
            push_log(
                &mut status,
                format!(
                    "libp2p dial пропущен: выбран Bore-compatible reverse tunnel, TCP handshake подтверждён до {}.",
                    endpoint.as_socket_label()
                ),
            );
            let _ = app.emit(
                "connection_success",
                ConnectionSuccessPayload {
                    addr: endpoint.as_multiaddr(),
                    transport: "reverse-tunnel".into(),
                },
            );
        } else {
            runtime
                .command_tx
                .send(SwarmCommand::DialPeer { peer_id, addrs })
                .await
                .context("не удалось отправить dial-команду в swarm")?;
        }

        Ok(())
    }

    pub async fn kick_peer(&self, peer_id: String) -> Result<()> {
        let peer_id = PeerId::from_str(&peer_id).context("РЅРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ PeerId")?;
        let runtime_guard = self.runtime.lock().await;
        let runtime = runtime_guard
            .as_ref()
            .context("РЅРµС‚ Р°РєС‚РёРІРЅРѕР№ swarm-СЃРµСЃСЃРёРё")?;

        if runtime.mode != SessionMode::Host {
            return Err(anyhow!("kick_peer РґРѕСЃС‚СѓРїРµРЅ С‚РѕР»СЊРєРѕ РґР»СЏ С…РѕСЃС‚Р°"));
        }

        runtime
            .command_tx
            .send(SwarmCommand::KickPeer { peer_id })
            .await
            .context("РЅРµ СѓРґР°Р»РѕСЃСЊ РѕС‚РїСЂР°РІРёС‚СЊ РєРѕРјР°РЅРґСѓ disconnect_peer_id")?;

        Ok(())
    }
}

async fn spawn_swarm_runtime(
    app: AppHandle,
    status: Arc<RwLock<NetworkStatus>>,
    config: SwarmSessionConfig,
) -> Result<(SwarmRuntime, SwarmBootstrap)> {
    let relay_bootstraps = load_relay_bootstraps()?;
    let relay_addr_strings = relay_bootstraps
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if relay_addr_strings.is_empty() {
        let mut status_guard = status.write().await;
        push_log(
            &mut status_guard,
            format!(
                "Переменная {} пуста. Relay fallback будет доступен только после конфигурации bootstrap-узлов.",
                RELAY_BOOTSTRAPS_ENV
            ),
        );
    } else {
        let mut status_guard = status.write().await;
        push_log(
            &mut status_guard,
            format!(
                "Р—Р°РіСЂСѓР¶РµРЅРѕ {} relay bootstrap-СѓР·Р»РѕРІ РґР»СЏ Circuit Relay v2.",
                relay_addr_strings.len()
            ),
        );
        push_log(
            &mut status_guard,
            "DNS resolver РїРµСЂРµРєР»СЋС‡РµРЅ РЅР° Google Public DNS (8.8.8.8 / 8.8.4.4), С‡С‚РѕР±С‹ РЅРµ Р·Р°РІРёСЃРµС‚СЊ РѕС‚ DNS РїСЂРѕРІР°Р№РґРµСЂР°.",
        );
    }

    let mut swarm = build_swarm().await?;
    let local_peer_id = *swarm.local_peer_id();
    let mut stream_control = swarm.behaviour().stream.new_control();

    for relay_addr in &relay_bootstraps {
        if let Some(peer_id) = peer_id_from_multiaddr(relay_addr) {
            swarm
                .behaviour_mut()
                .autonat
                .add_server(peer_id, Some(relay_addr.clone()));
        }
    }

    swarm
        .listen_on("/ip4/0.0.0.0/tcp/0".parse()?)
        .context("РЅРµ СѓРґР°Р»РѕСЃСЊ РѕС‚РєСЂС‹С‚СЊ libp2p TCP listener")?;
    let initial_listen_addrs = current_listen_addrs(&swarm);

    let cancel = CancellationToken::new();
    let active_route = Arc::new(RwLock::new(None));
    let (command_tx, command_rx) = mpsc::channel(32);
    let (ready_tx, ready_rx) = oneshot::channel();
    let mut handles = Vec::new();

    if config.mode == SessionMode::Host {
        let incoming = stream_control
            .accept(StreamProtocol::new(MINECRAFT_STREAM_PROTOCOL))
            .context("РЅРµ СѓРґР°Р»РѕСЃСЊ Р·Р°СЂРµРіРёСЃС‚СЂРёСЂРѕРІР°С‚СЊ inbound stream handler")?;
        let app_handle = app.clone();
        let status_handle = status.clone();
        let cancel_handle = cancel.clone();
        let local_port = config
            .host_local_port
            .context("РґР»СЏ host mode РѕР±СЏР·Р°С‚РµР»РµРЅ Р»РѕРєР°Р»СЊРЅС‹Р№ РїРѕСЂС‚ Minecraft")?;
        handles.push(tokio::spawn(async move {
            run_host_stream_acceptor(app_handle, status_handle, cancel_handle, incoming, local_port)
                .await;
        }));
    } else {
        let app_handle = app.clone();
        let status_handle = status.clone();
        let cancel_handle = cancel.clone();
        let control_handle = stream_control.clone();
        let active_route_handle = active_route.clone();
        handles.push(tokio::spawn(async move {
            run_client_proxy_listener(
                app_handle,
                status_handle,
                cancel_handle,
                control_handle,
                active_route_handle,
            )
            .await;
        }));
    }

    let app_handle = app.clone();
    let status_handle = status.clone();
    let cancel_handle = cancel.clone();
    let config_handle = config.clone();
    handles.push(tokio::spawn(async move {
        run_swarm_actor(
            swarm,
            app_handle,
            status_handle,
            cancel_handle,
            command_rx,
            ready_tx,
            config_handle,
            relay_bootstraps,
        )
        .await;
    }));

    let bootstrap = match timeout(Duration::from_secs(BOOTSTRAP_READY_TIMEOUT_SECS), ready_rx).await {
        Ok(Ok(bootstrap)) => bootstrap,
        Ok(Err(_)) | Err(_) => SwarmBootstrap {
            peer_id: local_peer_id.to_string(),
            listen_addrs: initial_listen_addrs,
            relay_addrs: relay_addr_strings,
            nat_status: "unknown".into(),
            local_game_port: config.host_local_port,
        },
    };

    Ok((
        SwarmRuntime {
            mode: config.mode,
            active_route,
            command_tx,
            cancel,
            handles,
        },
        bootstrap,
    ))
}

async fn build_swarm() -> Result<Swarm<ConnectorBehaviour>> {
    let mut dns_opts = dns::ResolverOpts::default();
    dns_opts.attempts = 2;
    dns_opts.timeout = Duration::from_secs(4);

    let swarm = SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            tcp::Config::default().nodelay(true),
            (tls::Config::new, noise::Config::new),
            yamux::Config::default,
        )?
        .with_dns_config(dns::ResolverConfig::google(), dns_opts)
        .with_websocket(
            (libp2p::tls::Config::new, libp2p::noise::Config::new),
            libp2p::yamux::Config::default,
        )
        .await?
        .with_relay_client(
            (libp2p::tls::Config::new, libp2p::noise::Config::new),
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|key: &libp2p::identity::Keypair, relay| {
            let local_peer_id = key.public().to_peer_id();
            Ok(ConnectorBehaviour {
                relay,
                dcutr: dcutr::Behaviour::new(local_peer_id),
                identify: identify::Behaviour::new(identify::Config::new(
                    APP_PROTOCOL.into(),
                    key.public(),
                )),
                autonat: autonat::Behaviour::new(local_peer_id, Default::default()),
                stream: libp2p_stream::Behaviour::new(),
            })
        })?
        .build();

    Ok(swarm)
}

async fn run_swarm_actor(
    mut swarm: Swarm<ConnectorBehaviour>,
    app: AppHandle,
    status: Arc<RwLock<NetworkStatus>>,
    cancel: CancellationToken,
    mut command_rx: mpsc::Receiver<SwarmCommand>,
    ready_tx: oneshot::Sender<SwarmBootstrap>,
    config: SwarmSessionConfig,
    relay_bootstraps: Vec<Multiaddr>,
) {
    let relay_peer_ids = relay_bootstraps
        .iter()
        .filter_map(peer_id_from_multiaddr)
        .collect::<HashSet<_>>();
    let mut relay_reservations = HashSet::new();
    let mut ready_tx = Some(ready_tx);
    let mut reverse_tunnel_handle: Option<ReverseTunnelHandle> = None;
    let mut reverse_tunnel_deadline: Option<Pin<Box<tokio::time::Sleep>>> =
        (config.mode == SessionMode::Host).then(|| {
            Box::pin(tokio::time::sleep(Duration::from_secs(
                REVERSE_TUNNEL_FALLBACK_SECS,
            )))
        });

    if !relay_bootstraps.is_empty() {
        if let Err(error) = ensure_relay_reservations(
            &mut swarm,
            &relay_bootstraps,
            &mut relay_reservations,
            &status,
        )
        .await
        {
            log_status(&status, format!("РќРµ СѓРґР°Р»РѕСЃСЊ Р·Р°СЂР°РЅРµРµ Р·Р°РїСЂРѕСЃРёС‚СЊ relay reservation: {error:#}"))
                .await;
        }
    }

    {
        let mut status_guard = status.write().await;
        status_guard.state = if config.mode == SessionMode::Host {
            ConnectionState::WaitingForPeer
        } else {
            ConnectionState::Idle
        };
        if let Some(version) = &config.minecraft_version {
            status_guard.minecraft_version = Some(version.clone());
        }
        status_guard.password_protected = config.password_protected;
        status_guard.note = Some(match config.mode {
            SessionMode::Host => format!(
                "РҐРѕСЃС‚ \"{}\" Р°РєС‚РёРІРµРЅ. Swarm РёСЃРїРѕР»СЊР·СѓРµС‚ TCP + WSS/443 relay transport. РџСѓР±Р»РёРєСѓР№С‚Рµ PeerId Рё Multiaddr РІ Ably.",
                config
                    .room_name
                    .as_deref()
                    .unwrap_or("Р‘РµР·С‹РјСЏРЅРЅР°СЏ РєРѕРјРЅР°С‚Р°")
            ),
            SessionMode::Client => {
                format!(
                    "Swarm РіРѕС‚РѕРІ. Minecraft-РєР»РёРµРЅС‚ РґРѕР»Р¶РµРЅ РїРѕРґРєР»СЋС‡Р°С‚СЊСЃСЏ Рє {}.",
                    CLIENT_LOCAL_BIND_ADDR
                )
            }
            SessionMode::Idle => String::new(),
        });
        if config.mode == SessionMode::Host {
            push_log(
                &mut status_guard,
                format!(
                    "Host metadata: room={}, password={}, version={}",
                    config
                        .room_name
                        .as_deref()
                        .unwrap_or("n/a"),
                    if config.password_protected { "on" } else { "off" },
                    config
                        .minecraft_version
                        .as_deref()
                        .unwrap_or("unknown")
                ),
            );
        }
    }

    loop {
        tokio::select! {
            _ = cancel.cancelled() => {
                break;
            }
            _ = async {
                if let Some(deadline) = &mut reverse_tunnel_deadline {
                    deadline.as_mut().await;
                } else {
                    futures::future::pending::<()>().await;
                }
            }, if reverse_tunnel_deadline.is_some() => {
                reverse_tunnel_deadline = None;
                if ready_tx.is_some() && reverse_tunnel_handle.is_none() {
                    let local_peer_id = *swarm.local_peer_id();
                    let listen_addrs = current_listen_addrs(&swarm);
                    match start_host_reverse_tunnel(
                        &app,
                        &status,
                        &config,
                        &cancel,
                        &mut ready_tx,
                        local_peer_id,
                        listen_addrs,
                    ).await {
                        Ok(handle) => {
                            reverse_tunnel_handle = Some(handle);
                        }
                        Err(error) => {
                            log_status(&status, format!("Reverse tunnel fallback не поднялся: {error:#}")).await;
                        }
                    }
                }
            }
            maybe_cmd = command_rx.recv() => {
                match maybe_cmd {
                    Some(command) => {
                        if let Err(error) = handle_swarm_command(
                            &mut swarm,
                            &status,
                            command,
                        ).await {
                            log_status(&status, format!("РљРѕРјР°РЅРґР° swarm Р·Р°РІРµСЂС€РёР»Р°СЃСЊ РѕС€РёР±РєРѕР№: {error:#}")).await;
                        }
                    }
                    None => break,
                }
            }
            event = swarm.select_next_some() => {
                if let Err(error) = handle_swarm_event(
                    &mut swarm,
                    &app,
                    &status,
                    &config,
                    &relay_bootstraps,
                    &relay_peer_ids,
                    &mut relay_reservations,
                    &mut ready_tx,
                    event,
                ).await {
                    log_status(&status, format!("РћС€РёР±РєР° РѕР±СЂР°Р±РѕС‚РєРё SwarmEvent: {error:#}")).await;
                }
            }
        }
    }

    if let Some(handle) = reverse_tunnel_handle.as_ref() {
        handle.abort();
    }
}

async fn handle_swarm_command(
    swarm: &mut Swarm<ConnectorBehaviour>,
    status: &Arc<RwLock<NetworkStatus>>,
    command: SwarmCommand,
) -> Result<()> {
    match command {
        SwarmCommand::DialPeer { peer_id, addrs } => {
            for addr in &addrs {
                swarm.add_peer_address(peer_id, addr.clone());
            }

            let opts = DialOpts::peer_id(peer_id)
                .condition(PeerCondition::DisconnectedAndNotDialing)
                .addresses(addrs.clone())
                .build();

            swarm
                .dial(opts)
                .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ dial'РёС‚СЊ peer {peer_id}"))?;

            let mut status_guard = status.write().await;
            status_guard.state = ConnectionState::Connecting;
            push_log(
                &mut status_guard,
                format!("Dial РЅР°С‡Р°С‚ РґР»СЏ peer {peer_id} С‡РµСЂРµР· {} Р°РґСЂРµСЃ(Р°).", addrs.len()),
            );
        }
        SwarmCommand::KickPeer { peer_id } => {
            swarm
                .disconnect_peer_id(peer_id)
                .map_err(|_| anyhow!("peer {peer_id} РЅРµ РїРѕРґРєР»СЋС‡С‘РЅ"))?;
            let mut status_guard = status.write().await;
            push_log(
                &mut status_guard,
                format!("Peer {peer_id} РѕС‚РєР»СЋС‡С‘РЅ С‡РµСЂРµР· disconnect_peer_id()."),
            );
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_swarm_event(
    swarm: &mut Swarm<ConnectorBehaviour>,
    app: &AppHandle,
    status: &Arc<RwLock<NetworkStatus>>,
    config: &SwarmSessionConfig,
    relay_bootstraps: &[Multiaddr],
    relay_peer_ids: &HashSet<PeerId>,
    relay_reservations: &mut HashSet<String>,
    ready_tx: &mut Option<oneshot::Sender<SwarmBootstrap>>,
    event: SwarmEvent<ConnectorEvent>,
) -> Result<()> {
    match event {
        SwarmEvent::NewListenAddr { address, .. } => {
            let address_string = address.to_string();
            let mut status_guard = status.write().await;
            merge_listen_addr(&mut status_guard, &address_string);
            if is_relay_addr(&address) {
                status_guard.transport_path = Some("relay-reservation".into());
                push_log(
                    &mut status_guard,
                    format!("Relay reservation Р°РєС‚РёРІРµРЅ: {address_string}"),
                );
                let _ = app.emit(
                    "relay_active",
                    RelayActivePayload {
                        relay_addr: address_string.clone(),
                    },
                );
            } else {
                push_log(
                    &mut status_guard,
                    format!("Swarm СЃР»СѓС€Р°РµС‚ РЅР° {address_string}"),
                );
            }

            if should_publish_bootstrap_addr(&address, relay_bootstraps) {
                if let Some(sender) = ready_tx.take() {
                    let listen_addrs = current_listen_addrs(swarm);
                    let _ = sender.send(SwarmBootstrap {
                        peer_id: swarm.local_peer_id().to_string(),
                        listen_addrs,
                        relay_addrs: relay_bootstraps.iter().map(ToString::to_string).collect(),
                        nat_status: status_guard
                            .note
                            .clone()
                            .unwrap_or_else(|| "unknown".into()),
                        local_game_port: config.host_local_port,
                    });
                }
            }
        }
        SwarmEvent::ExternalAddrConfirmed { address } => {
            let mut status_guard = status.write().await;
            status_guard.public_udp_addr = Some(address.to_string());
            push_log(
                &mut status_guard,
                format!("РџРѕРґС‚РІРµСЂР¶РґС‘РЅ РІРЅРµС€РЅРёР№ Р°РґСЂРµСЃ swarm: {address}"),
            );
        }
        SwarmEvent::ConnectionEstablished { peer_id, endpoint, .. } => {
            let addr = connected_point_addr(&endpoint).to_string();
            let relayed = endpoint.is_relayed();

            if relay_peer_ids.contains(&peer_id) {
                let mut status_guard = status.write().await;
                push_log(
                    &mut status_guard,
                    format!("РџРѕРґРєР»СЋС‡С‘РЅ relay bootstrap {peer_id} С‡РµСЂРµР· {addr}"),
                );
                if relayed {
                    let _ = app.emit(
                        "relay_active",
                        RelayActivePayload {
                            relay_addr: addr,
                        },
                    );
                }
            } else {
                let mut status_guard = status.write().await;
                upsert_peer(&mut status_guard, peer_id, &addr, true);
                status_guard.state = if config.mode == SessionMode::Host {
                    ConnectionState::Hosting
                } else {
                    ConnectionState::Connected
                };
                status_guard.transport_path = Some(if relayed {
                    "relay-circuit".into()
                } else {
                    "direct".into()
                });
                status_guard.note = Some(if config.mode == SessionMode::Client {
                    format!(
                        "РўСѓРЅРЅРµР»СЊ Р°РєС‚РёРІРµРЅ. РџРѕРґРєР»СЋС‡Р°Р№С‚РµСЃСЊ РІ Minecraft Рє {}.",
                        CLIENT_LOCAL_BIND_ADDR
                    )
                } else {
                    "Peer РїРѕРґРєР»СЋС‡С‘РЅ Рє РІР°С€РµРјСѓ С…РѕСЃС‚Сѓ С‡РµСЂРµР· libp2p stream.".into()
                });
                push_log(
                    &mut status_guard,
                    format!(
                        "Peer {peer_id} РїРѕРґРєР»СЋС‡С‘РЅ С‡РµСЂРµР· {} ({addr})",
                        if relayed { "relay" } else { "direct" }
                    ),
                );
                let _ = app.emit(
                    "peer_connected",
                    PeerConnectedPayload {
                        peer_id: peer_id.to_string(),
                        addr,
                        relayed,
                    },
                );
            }
        }
        SwarmEvent::ConnectionClosed { peer_id, cause, .. } => {
            let mut status_guard = status.write().await;
            mark_peer_disconnected(&mut status_guard, peer_id);
            if !relay_peer_ids.contains(&peer_id) {
                status_guard.state = if config.mode == SessionMode::Host {
                    ConnectionState::WaitingForPeer
                } else {
                    ConnectionState::Idle
                };
                push_log(
                    &mut status_guard,
                    format!(
                        "РЎРѕРµРґРёРЅРµРЅРёРµ СЃ peer {peer_id} Р·Р°РєСЂС‹С‚Рѕ{}",
                        cause
                            .as_ref()
                            .map(|error| format!(": {error}"))
                            .unwrap_or_default()
                    ),
                );
            }
        }
        SwarmEvent::OutgoingConnectionError { peer_id, error, .. } => {
            let mut status_guard = status.write().await;
            let is_relay_bootstrap_error = peer_id
                .map(|peer| relay_peer_ids.contains(&peer))
                .unwrap_or(false);

            if is_relay_bootstrap_error {
                push_log(
                    &mut status_guard,
                    format!("Dial error for relay bootstrap {}: {error}", peer_id.unwrap()),
                );
                status_guard.last_error = None;
                if config.mode == SessionMode::Host {
                    status_guard.state = ConnectionState::WaitingForPeer;
                } else if matches!(status_guard.state, ConnectionState::Error | ConnectionState::Starting) {
                    status_guard.state = ConnectionState::Idle;
                }
            } else {
                status_guard.state = ConnectionState::Error;
                status_guard.last_error = Some(error.to_string());
                push_log(
                    &mut status_guard,
                    format!(
                        "Dial error{}: {error}",
                        peer_id
                            .map(|peer| format!(" for {peer}"))
                            .unwrap_or_default()
                    ),
                );
            }
        }
        SwarmEvent::IncomingConnectionError { error, .. } => {
            log_status(status, format!("Incoming connection error: {error}")).await;
        }
        SwarmEvent::Behaviour(ConnectorEvent::Autonat(autonat::Event::StatusChanged { new, .. })) => {
            {
                let mut status_guard = status.write().await;
                status_guard.note = Some(format!("AutoNAT: {}", nat_status_label(&new)));
                push_log(
                    &mut status_guard,
                    format!("AutoNAT СЃС‚Р°С‚СѓСЃ РёР·РјРµРЅРёР»СЃСЏ: {}", nat_status_label(&new)),
                );
                if let autonat::NatStatus::Public(addr) = &new {
                    status_guard.public_udp_addr = Some(addr.to_string());
                }
            }

            if matches!(new, autonat::NatStatus::Private | autonat::NatStatus::Unknown) {
                ensure_relay_reservations(swarm, relay_bootstraps, relay_reservations, status).await?;
            }
        }
        SwarmEvent::Behaviour(ConnectorEvent::Dcutr(event)) => {
            match event.result {
                Ok(_) => {
                    {
                        let mut status_guard = status.write().await;
                        status_guard.transport_path = Some("direct-hole-punch".into());
                        push_log(
                            &mut status_guard,
                            format!("DCUtR hole punch СѓСЃРїРµС€РµРЅ РґР»СЏ peer {}.", event.remote_peer_id),
                        );
                    }
                    let _ = app.emit(
                        "hole_punch_success",
                        HolePunchPayload {
                            peer_id: event.remote_peer_id.to_string(),
                        },
                    );
                }
                Err(error) => {
                    log_status(
                        status,
                        format!("DCUtR hole punch РґР»СЏ {} РЅРµ СѓРґР°Р»СЃСЏ: {error}", event.remote_peer_id),
                    )
                    .await;
                }
            }
        }
        SwarmEvent::Behaviour(ConnectorEvent::Identify(identify::Event::Received {
            peer_id,
            info,
            ..
        })) => {
            for addr in info.listen_addrs {
                swarm.add_peer_address(peer_id, addr);
            }
            log_status(status, format!("РџРѕР»СѓС‡РµРЅ identify РѕС‚ peer {peer_id}.")).await;
        }
        SwarmEvent::Behaviour(ConnectorEvent::Relay(event)) => {
            log_status(status, format!("Relay client event: {event:?}")).await;
        }
        SwarmEvent::Behaviour(ConnectorEvent::Stream(())) => {}
        other => {
            tracing::debug!("Unhandled swarm event: {other:?}");
        }
    }

    Ok(())
}

async fn ensure_relay_reservations(
    swarm: &mut Swarm<ConnectorBehaviour>,
    relay_bootstraps: &[Multiaddr],
    relay_reservations: &mut HashSet<String>,
    status: &Arc<RwLock<NetworkStatus>>,
) -> Result<()> {
    for relay_addr in relay_bootstraps {
        let relay_key = relay_addr.to_string();
        if relay_reservations.contains(&relay_key) {
            continue;
        }

        swarm
            .dial(relay_addr.clone())
            .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ dial'РёС‚СЊ relay {relay_addr}"))?;

        let mut reservation_addr = relay_addr.clone();
        reservation_addr.push(Protocol::P2pCircuit);
        swarm
            .listen_on(reservation_addr.clone())
            .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ Р·Р°РїСЂРѕСЃРёС‚СЊ reservation С‡РµСЂРµР· {relay_addr}"))?;

        relay_reservations.insert(relay_key.clone());

        let mut status_guard = status.write().await;
        push_log(
            &mut status_guard,
            format!("Р—Р°РїСЂРѕС€РµРЅ Circuit Relay reservation С‡РµСЂРµР· {relay_key}"),
        );
    }

    Ok(())
}

async fn start_host_reverse_tunnel(
    app: &AppHandle,
    status: &Arc<RwLock<NetworkStatus>>,
    config: &SwarmSessionConfig,
    cancel: &CancellationToken,
    ready_tx: &mut Option<oneshot::Sender<SwarmBootstrap>>,
    local_peer_id: PeerId,
    existing_listen_addrs: Vec<String>,
) -> Result<ReverseTunnelHandle> {
    let local_port = config
        .host_local_port
        .context("reverse tunnel fallback доступен только в host mode")?;
    if let Err(error) = verify_local_minecraft_target(local_port).await {
        let mut status_guard = status.write().await;
        status_guard.last_error = Some(error.to_string());
        status_guard.note = Some(format!(
            "Локальный Minecraft на 127.0.0.1:{local_port} недоступен. Откройте мир в LAN или запустите сервер перед публикацией комнаты."
        ));
        push_log(
            &mut status_guard,
            format!(
                "Reverse tunnel не опубликован: локальный Minecraft на 127.0.0.1:{local_port} недоступен: {error:#}"
            ),
        );
        return Err(error).with_context(|| format!("локальный Minecraft на 127.0.0.1:{local_port} недоступен"));
    }
    let handle = tunnel::start_reverse_tunnel(
        ReverseTunnelConfig::bore_pub(local_port),
        cancel.clone(),
    )
    .await
    .context("РЅРµ СѓРґР°Р»РѕСЃСЊ РѕС‚РєСЂС‹С‚СЊ Bore-compatible reverse tunnel")?;

    let endpoint = handle.endpoint().clone();
    let socket_label = endpoint.as_socket_label();
    let endpoint_multiaddr = endpoint.as_multiaddr();

    {
        let mut status_guard = status.write().await;
        status_guard.transport_path = Some("reverse-tunnel".into());
        status_guard.public_udp_addr = Some(socket_label.clone());
        status_guard.note = Some(format!(
            "libp2p relay не ответил вовремя. Активирован reverse tunnel fallback через {}.",
            socket_label
        ));
        push_log(
            &mut status_guard,
            format!("Bore-compatible reverse tunnel поднят: {}", socket_label),
        );
    }

    let _ = app.emit(
        "reverse_tunnel_ready",
        ReverseTunnelReadyPayload {
            endpoint: socket_label.clone(),
            multiaddr: endpoint_multiaddr.clone(),
        },
    );

    if let Some(sender) = ready_tx.take() {
        let mut listen_addrs = existing_listen_addrs;
        listen_addrs.push(endpoint_multiaddr);
        listen_addrs.sort();
        listen_addrs.dedup();

        let _ = sender.send(SwarmBootstrap {
            peer_id: local_peer_id.to_string(),
            listen_addrs,
            relay_addrs: Vec::new(),
            nat_status: "reverse-tunnel".into(),
            local_game_port: config.host_local_port,
        });
    }

    Ok(handle)
}

async fn run_host_stream_acceptor(
    app: AppHandle,
    status: Arc<RwLock<NetworkStatus>>,
    cancel: CancellationToken,
    mut incoming: IncomingStreams,
    local_game_port: u16,
) {
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            inbound = incoming.next() => {
                let Some((peer_id, stream)) = inbound else {
                    break;
                };

                let app_handle = app.clone();
                let status_handle = status.clone();
                tokio::spawn(async move {
                    if let Err(error) = pipe_inbound_minecraft_stream(stream, local_game_port).await {
                        log_status(
                            &status_handle,
                            format!("Inbound stream РѕС‚ {peer_id} Р·Р°РІРµСЂС€РёР»СЃСЏ РѕС€РёР±РєРѕР№: {error:#}"),
                        )
                        .await;
                    } else {
                        let _ = app_handle.emit(
                            "peer_connected",
                            PeerConnectedPayload {
                                peer_id: peer_id.to_string(),
                                addr: format!("127.0.0.1:{local_game_port}"),
                                relayed: false,
                            },
                        );
                    }
                });
            }
        }
    }
}

async fn run_client_proxy_listener(
    _app: AppHandle,
    status: Arc<RwLock<NetworkStatus>>,
    cancel: CancellationToken,
    stream_control: StreamControl,
    active_route: Arc<RwLock<Option<ActiveRoute>>>,
) {
    let listener = match TcpListener::bind(CLIENT_LOCAL_BIND_ADDR).await {
        Ok(listener) => listener,
        Err(error) => {
            log_status(
                &status,
                format!("РќРµ СѓРґР°Р»РѕСЃСЊ РѕС‚РєСЂС‹С‚СЊ Р»РѕРєР°Р»СЊРЅС‹Р№ Minecraft proxy РЅР° {CLIENT_LOCAL_BIND_ADDR}: {error}"),
            )
            .await;
            return;
        }
    };

    log_status(
        &status,
        format!("Р›РѕРєР°Р»СЊРЅС‹Р№ Minecraft proxy СЃР»СѓС€Р°РµС‚ РЅР° {CLIENT_LOCAL_BIND_ADDR}"),
    )
    .await;

    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            accept_result = listener.accept() => {
                match accept_result {
                    Ok((tcp_stream, addr)) => {
                        let current_route = active_route.read().await.clone();
                        let Some(route) = current_route else {
                            log_status(
                                &status,
                                format!("Minecraft TCP-РєР»РёРµРЅС‚ {addr} РїРѕРґРєР»СЋС‡РёР»СЃСЏ СЂР°РЅСЊС€Рµ, С‡РµРј РІС‹Р±СЂР°РЅ remote peer."),
                            )
                            .await;
                            continue;
                        };

                        let control = stream_control.clone();
                        let status_handle = status.clone();
                        tokio::spawn(async move {
                            match route {
                                ActiveRoute::Libp2pPeer(peer_id) => {
                                    if let Err(error) = open_and_pipe_outbound_stream(control, peer_id, tcp_stream).await {
                                        log_status(
                                            &status_handle,
                                            format!("РћС€РёР±РєР° outbound /mc-p2p/1.0.0 РґР»СЏ {peer_id}: {error:#}"),
                                        )
                                        .await;
                                    }
                                }
                                ActiveRoute::ReverseTunnel(endpoint) => {
                                    if let Err(error) = tunnel::bridge_tcp_to_remote(
                                        tcp_stream,
                                        &endpoint.public_host,
                                        endpoint.public_port,
                                    )
                                    .await
                                    {
                                        log_status(
                                            &status_handle,
                                            format!(
                                                "РћС€РёР±РєР° reverse tunnel client bridge РґР»СЏ {}: {error:#}",
                                                endpoint.as_socket_label()
                                            ),
                                        )

                                        .await;
                                    }
                                }
                            }
                        });
                    }
                    Err(error) => {
                        log_status(&status, format!("РћС€РёР±РєР° accept() РЅР° Р»РѕРєР°Р»СЊРЅРѕРј proxy: {error}")).await;
                    }
                }
            }
        }
    }
}

async fn open_and_pipe_outbound_stream(
    mut stream_control: StreamControl,
    peer_id: PeerId,
    tcp_stream: TcpStream,
) -> Result<()> {
    let stream = stream_control
        .open_stream(peer_id, StreamProtocol::new(MINECRAFT_STREAM_PROTOCOL))
        .await
        .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ РѕС‚РєСЂС‹С‚СЊ libp2p stream Рє {peer_id}"))?;

    pipe_bidirectional(tcp_stream, stream)
        .await
        .with_context(|| format!("copy_bidirectional Рє {peer_id} Р·Р°РІРµСЂС€РёР»СЃСЏ РѕС€РёР±РєРѕР№"))?;

    Ok(())
}

async fn pipe_inbound_minecraft_stream(stream: Stream, local_game_port: u16) -> Result<()> {
    let target = format!("127.0.0.1:{local_game_port}");
    let tcp_stream = TcpStream::connect(&target)
        .await
        .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ РїРѕРґРєР»СЋС‡РёС‚СЊСЃСЏ Рє Р»РѕРєР°Р»СЊРЅРѕРјСѓ Minecraft РЅР° {target}"))?;

    pipe_bidirectional(tcp_stream, stream)
        .await
        .with_context(|| format!("bridge РЅР° Р»РѕРєР°Р»СЊРЅС‹Р№ Minecraft {target} Р·Р°РІРµСЂС€РёР»СЃСЏ РѕС€РёР±РєРѕР№"))?;

    Ok(())
}

async fn pipe_bidirectional(mut tcp_stream: TcpStream, p2p_stream: Stream) -> Result<()> {
    let mut p2p_stream = p2p_stream.compat();
    let (uploaded, downloaded) = tokio::io::copy_bidirectional(&mut tcp_stream, &mut p2p_stream)
        .await
        .context("tokio::io::copy_bidirectional вернул ошибку")?;
    tracing::debug!("Minecraft tunnel bytes: up={uploaded}, down={downloaded}");
    Ok(())
}

async fn verify_local_minecraft_target(local_port: u16) -> Result<()> {
    let target = ("127.0.0.1", local_port);
    let stream = timeout(Duration::from_secs(2), TcpStream::connect(target))
        .await
        .with_context(|| format!("таймаут при проверке локального Minecraft на 127.0.0.1:{local_port}"))?
        .with_context(|| format!("локальный Minecraft на 127.0.0.1:{local_port} не принимает TCP-подключение"))?;
    stream
        .writable()
        .await
        .with_context(|| format!("локальный Minecraft на 127.0.0.1:{local_port} не стал writable"))?;
    Ok(())
}

async fn probe_reverse_tunnel_endpoint(endpoint: &ReverseTunnelEndpoint) -> Result<()> {
    let connect_result = timeout(
        Duration::from_secs(5),
        TcpStream::connect((endpoint.public_host.as_str(), endpoint.public_port)),
    )
    .await
    .context("reverse tunnel TCP handshake timed out")?;

    let stream = connect_result.with_context(|| {
        format!(
            "failed to open TCP connection to {}",
            endpoint.as_socket_label()
        )
    })?;
    stream
        .writable()
        .await
        .with_context(|| format!("TCP stream to {} never became writable", endpoint.as_socket_label()))?;
    Ok(())
}

fn default_status() -> NetworkStatus {
    NetworkStatus {
        signaling_server: SIGNALING_LABEL.into(),
        ..NetworkStatus::default()
    }
}

fn push_log(status: &mut NetworkStatus, message: impl Into<String>) {
    status.logs.push(message.into());
    if status.logs.len() > MAX_LOG_LINES {
        let overflow = status.logs.len() - MAX_LOG_LINES;
        status.logs.drain(0..overflow);
    }
}

async fn log_status(status: &Arc<RwLock<NetworkStatus>>, message: impl Into<String>) {
    let mut status_guard = status.write().await;
    push_log(&mut status_guard, message.into());
}

fn merge_listen_addr(status: &mut NetworkStatus, address: &str) {
    if status.udp_bind_addr.is_none() {
        status.udp_bind_addr = Some(address.to_owned());
    } else if status.public_udp_addr.is_none() && !address.contains("/ip4/127.0.0.1/") {
        status.public_udp_addr = Some(address.to_owned());
    }
}

fn upsert_peer(status: &mut NetworkStatus, peer_id: PeerId, addr: &str, connected: bool) {
    if let Some(existing) = status
        .peers
        .iter_mut()
        .find(|entry| entry.peer_id == peer_id.to_string())
    {
        existing.addr = addr.to_owned();
        existing.connected = connected;
    } else {
        status.peers.push(PeerInfo {
            peer_id: peer_id.to_string(),
            addr: addr.to_owned(),
            connected,
            ping_ms: None,
        });
    }

    status.peers.retain(|peer| peer.connected);
    status.peer_count = status.peers.len();
}

fn mark_peer_disconnected(status: &mut NetworkStatus, peer_id: PeerId) {
    status.peers.retain(|peer| peer.peer_id != peer_id.to_string());
    status.peer_count = status.peers.len();
}

fn load_relay_bootstraps() -> Result<Vec<Multiaddr>> {
    let mut values = DEFAULT_RELAY_BOOTSTRAPS
        .iter()
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>();
    let value = std::env::var(RELAY_BOOTSTRAPS_ENV).unwrap_or_default();
    values.extend(
        value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(ToOwned::to_owned),
    );

    values.sort();
    values.dedup();

    values
        .into_iter()
        .map(|entry| entry.trim().to_owned())
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            entry
                .parse::<Multiaddr>()
                .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ СЂР°СЃРїР°СЂСЃРёС‚СЊ relay multiaddr: {entry}"))
        })
        .collect()
}

fn current_listen_addrs(swarm: &Swarm<ConnectorBehaviour>) -> Vec<String> {
    swarm
        .listeners()
        .map(ToString::to_string)
        .filter(|addr| {
            addr.contains("/p2p-circuit")
                || (!addr.contains("/ip4/0.0.0.0/") && !addr.contains("/ip6/::/"))
        })
        .collect()
}

fn should_publish_bootstrap_addr(address: &Multiaddr, relay_bootstraps: &[Multiaddr]) -> bool {
    relay_bootstraps.is_empty() || is_relay_addr(address)
}

fn reverse_tunnel_target_from_addrs(addrs: &[Multiaddr]) -> Option<ReverseTunnelEndpoint> {
    addrs.iter().find_map(|addr| {
        let (host, port) = tcp_host_and_port_from_multiaddr(addr)?;
        if host.eq_ignore_ascii_case(DEFAULT_BORE_HOST) {
            Some(ReverseTunnelEndpoint {
                public_host: host,
                public_port: port,
            })
        } else {
            None
        }
    })
}

fn normalize_multiaddr_input(value: &str) -> Result<Multiaddr> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(anyhow!("РїСѓСЃС‚РѕР№ Р°РґСЂРµСЃ"));
    }

    if trimmed.starts_with('/') {
        return trimmed
            .parse::<Multiaddr>()
            .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ СЂР°СЃРїР°СЂСЃРёС‚СЊ multiaddr: {trimmed}"));
    }

    let (host, port) = split_host_port(trimmed)
        .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ СЂР°Р·РѕР±СЂР°С‚СЊ host:port Р°РґСЂРµСЃ: {trimmed}"))?;
    let multiaddr = socket_label_to_multiaddr(&host, port);
    multiaddr
        .parse::<Multiaddr>()
        .with_context(|| format!("РЅРµ СѓРґР°Р»РѕСЃСЊ РїСЂРµРѕР±СЂР°Р·РѕРІР°С‚СЊ РІ multiaddr: {multiaddr}"))
}

fn split_host_port(value: &str) -> Result<(String, u16)> {
    let (host, port_str) = value
        .rsplit_once(':')
        .ok_or_else(|| anyhow!("РѕР¶РёРґР°РµС‚СЃСЏ С„РѕСЂРјР°С‚ host:port"))?;
    let port = port_str
        .parse::<u16>()
        .with_context(|| format!("РЅРµРєРѕСЂСЂРµРєС‚РЅС‹Р№ РїРѕСЂС‚: {port_str}"))?;
    Ok((host.trim_matches(&['[', ']'][..]).to_string(), port))
}

fn socket_label_to_multiaddr(host: &str, port: u16) -> String {
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        format!("/ip4/{host}/tcp/{port}")
    } else if host.parse::<std::net::Ipv6Addr>().is_ok() {
        format!("/ip6/{host}/tcp/{port}")
    } else {
        format!("/dns4/{host}/tcp/{port}")
    }
}

fn peer_id_from_multiaddr(addr: &Multiaddr) -> Option<PeerId> {
    addr.iter().find_map(|protocol| match protocol {
        Protocol::P2p(peer_id) => Some(peer_id),
        _ => None,
    })
}

fn tcp_host_and_port_from_multiaddr(addr: &Multiaddr) -> Option<(String, u16)> {
    let mut host = None;
    let mut port = None;

    for protocol in addr.iter() {
        match protocol {
            Protocol::Dns(domain) | Protocol::Dns4(domain) | Protocol::Dns6(domain) => {
                host = Some(domain.to_string());
            }
            Protocol::Ip4(ip) => {
                host = Some(ip.to_string());
            }
            Protocol::Ip6(ip) => {
                host = Some(ip.to_string());
            }
            Protocol::Tcp(value) => {
                port = Some(value);
            }
            _ => {}
        }
    }

    match (host, port) {
        (Some(host), Some(port)) => Some((host, port)),
        _ => None,
    }
}

fn connected_point_addr(endpoint: &ConnectedPoint) -> &Multiaddr {
    match endpoint {
        ConnectedPoint::Dialer { address, .. } => address,
        ConnectedPoint::Listener { send_back_addr, .. } => send_back_addr,
    }
}

fn is_relay_addr(addr: &Multiaddr) -> bool {
    addr.iter().any(|protocol| matches!(protocol, Protocol::P2pCircuit))
}

fn nat_status_label(status: &autonat::NatStatus) -> &'static str {
    match status {
        autonat::NatStatus::Public(_) => "public",
        autonat::NatStatus::Private => "private",
        autonat::NatStatus::Unknown => "unknown",
    }
}
