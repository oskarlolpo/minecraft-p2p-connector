# Minecraft P2P Connector

`Minecraft P2P Connector` is a Windows desktop application for Minecraft Java Edition that lets players host and join LAN-style sessions over the Internet without requiring a classic VPN adapter as the primary user flow.

The project is built for harsh real-world networks:

- CGNAT
- blocked UDP/STUN
- unstable Wi-Fi
- broken ISP DNS
- users without white IPs or port forwarding

The product goal is simple:

- host a world
- join a friend
- connect Minecraft to `localhost:25565`

The application itself is responsible for all transport complexity.

## Current Direction

The repository is in transition from an older QUIC/libp2p-heavy experimental branch toward a cleaner transport stack:

1. direct connection first
2. free relay-compatible path second
3. mesh fallback for hostile networks

The next generation of the connector is being designed around:

- Rust backend
- Tauri desktop shell
- separate networking helper process
- event-driven UI state
- Minecraft localhost proxy model

## Cloudflare TURN Fallback

The repository now includes a real Cloudflare TURN/WebRTC fallback path in addition to the existing direct QUIC path.

Pieces:

- desktop runtime: [src-tauri/src/network/cloudflare_rtc.rs](G:/minecraftjava/p2p/src-tauri/src/network/cloudflare_rtc.rs)
- credential config: [src-tauri/src/network/cloudflare.rs](G:/minecraftjava/p2p/src-tauri/src/network/cloudflare.rs)
- worker backend: [apps/cloudflare-turn-worker](G:/minecraftjava/p2p/apps/cloudflare-turn-worker)
- deploy guide: [docs/cloudflare-turn-deploy.md](G:/minecraftjava/p2p/docs/cloudflare-turn-deploy.md)

The application only marks a room as `cloudflare_turn_ready` when the credential backend is actually reachable.

## Why This Exists

Most existing ways to play Minecraft over the Internet from a LAN world fail in one of two ways:

- they rely on users understanding port forwarding, NAT and public IPs
- they force a full VPN adapter into the system

This project tries to offer a third path:

- consumer-friendly
- low-latency
- transport-aware
- focused on Minecraft

## Product Principles

- no fake “connected” state before a real tunnel exists
- `localhost` remains the only Minecraft target the player needs to know
- networking and UI must be separated
- logging must explain failures clearly
- free-first strategy, not VPS-first strategy

## Repository Layout

Current active app:

- `src-tauri/` — Tauri backend and desktop shell integration
- `src/` — frontend UI

Research and planning:

- `docs/analysis/` — external project analysis
- `docs/architecture/` — target architecture and design direction
- `docs/releases/` — planned release notes and validation checklists

## Important Documents

- [Project Vision](G:/minecraftjava/p2p/docs/vision.md)
- [Russia-Wide Free Architecture](G:/minecraftjava/p2p/docs/architecture/russia-free-connector.md)
- [13 External Projects Analysis](G:/minecraftjava/p2p/docs/analysis/newrepo-13-projects-report.md)
- [Next Release Draft](G:/minecraftjava/p2p/docs/releases/p2p-connector-next-release.md)
- [Transport Roadmap](G:/minecraftjava/p2p/docs/roadmap.md)

## Long-Term Architecture

The target structure for the next major update is:

```text
G:\minecraftjava\p2p
├─ apps
│  ├─ tauri-ui
│  └─ net-helper
├─ crates
│  ├─ bp-protocol
│  ├─ bp-signaling
│  ├─ bp-webrtc
│  ├─ bp-yggstack
│  ├─ bp-proxy
│  └─ bp-observability
└─ docs
```

This structure is documented now even before the full migration is complete, because the previous approach mixed too much transport state into the UI-facing Tauri app.

## Build Status

The existing app remains buildable while the architecture is being cleaned up.

Typical checks:

```powershell
cd G:\minecraftjava\p2p
npm run build
cargo check --manifest-path src-tauri\Cargo.toml
```

## Scope of the Next Major Update

The next serious update is not a cosmetic patch. It is intended to:

- improve direct join success across Russia and neighboring regions
- reduce transport fragility
- keep the service free in its baseline mode
- provide a realistic fallback path for users behind hostile ISPs

## Status

Active redevelopment.

This repository should now be treated as:

- a buildable desktop prototype
- an architectural migration target
- the canonical place for the next production-ready connector generation
