# Minecraft P2P Connector: Project Snapshot (2026-04-09)

## What This Is

Desktop app that lets a Minecraft Java player expose a local LAN world or server to remote players without manual port-forwarding. It focuses on low-latency direct connectivity, with fallbacks when NAT traversal fails.

Core idea: run a local TCP proxy on the client (`localhost:25565`) that tunnels traffic to the host over a P2P transport (QUIC or other routes).

## Repository Layout

- `src-tauri/`: Tauri + Rust networking core (host/client sessions, tunnels, update installer logic).
- `src/`: Frontend (Vite + vanilla JS + Tailwind). UI, Ably presence lobby, localStorage profile/settings.
- `signaling-server/`: Rust WS + UDP signaling server (room code + UDP observed address + peer announcements).
- `.github/workflows/release.yml`: Windows bundle build on tag push.

## Runtime Flows (High-Level)

### Host

1. UI calls `start_hosting` (Tauri command).
2. Rust binds UDP (punch + transport), discovers public UDP addr via STUN.
3. Host advertises presence in Ably lobby (currently done in frontend).
4. Host accepts client handshake and establishes tunnel.
5. Optional: Geyser bridge enables Bedrock access with a local jar sidecar.

### Client

1. UI selects a host from lobby (presence list).
2. UI calls `connect_to_peer` with the chosen endpoint details.
3. Rust establishes tunnel and opens local proxy on `localhost:25565`.
4. User connects in Minecraft to `localhost`.

## Networking Components (Observed)

- Direct path: UDP hole punching + QUIC (`quinn`).
- Fallback path: Ably MQTT-based relay (Rust `rumqttc`).
- Alternate/next-gen path: libp2p swarm manager exists (`src-tauri/src/network/network_swarm.rs`) plus reverse tunnel concepts (Bore-like).
- STUN discovery: custom STUN parser in `src-tauri/src/signaling.rs`.

## Configuration Surface

### Signaling / NAT

- `MC_STUN_SERVERS`: comma-separated STUN servers.

### Signaling server (separate service)

- `SIGNAL_WS_ADDR`, `SIGNAL_UDP_ADDR`: bind addresses for `signaling-server/`.

### Relay (Ably MQTT)

- `ABLY_API_KEY` / `MC_ABLY_API_KEY`
- `MC_RELAY_MQTT_HOST`, `MC_RELAY_MQTT_PORT`, `MC_RELAY_TOPIC_PREFIX`

### App runtime (UI / Tauri)

- Various settings and profile data persist to localStorage (theme, language, avatar, nickname, external servers).

## Build & Release

- Frontend build: `npm run build` (Vite).
- Rust build checks: `cargo check --manifest-path src-tauri/Cargo.toml` and `cargo check --manifest-path signaling-server/Cargo.toml`.
- Release: GitHub Actions builds Windows `nsis` and `msi` bundles on tag push (`v*`) using `tauri-apps/tauri-action`.

## Current Gaps / Risks (Unfixed)

### Critical

- Embedded Ably API key in the shipped frontend (`src/main.js`) and as a default in Rust relay config.
  - This is effectively a secret shipped to every user; it can be abused and can create runaway costs and account compromise risk.
- QUIC client config disables TLS cert verification (`build_insecure_client_config`).
  - Without pinning/verifying a peer certificate fingerprint obtained through signaling, a MitM can impersonate hosts.
- Russian localization strings appear mojibake-corrupted in `src/i18n.js`.
  - UI text is unreadable; a runtime decoder exists, indicating a systemic encoding issue.

### High

- Documentation drift:
  - README mentions a local signaling server as the main path, but current UI relies on Ably presence/channels.
  - README claims "no relay fallback" while code includes an Ably relay.
- Update installer path: downloads and runs installer; integrity verification and signing strategy is not explicit.
- Tauri CSP is disabled (`csp: null`).

### Medium

- No automated tests for the networking edge cases (NAT types, relay fallback, version detection, UI flows).
- Operability: no structured telemetry, limited incident visibility (logs only).
- Abuse resistance: rate limiting and spam control are unclear for any public lobby path.

## What To Add Next (Suggested)

### Near-term (stabilize)

- Remove all embedded Ably secrets; switch to Ably token auth (minted by your server) or replace Ably with your own `signaling-server/`.
- Introduce certificate pinning for QUIC: host generates self-signed cert; cert fingerprint is delivered via signaling; client verifies it.
- Fix localization encoding at the source and remove the mojibake decoder hack once clean.
- Decide and document the authoritative transport stack:
  - QUIC-only + (TURN/relay) fallback
  - or libp2p swarm as the primary path
  - and remove dead/unwired paths.

### Later (product)

- Account system and private lobbies (invites, password policy, anti-scrape).
- Relay/TURN as a paid fallback for symmetric NAT (key monetization lever).
- Quality: end-to-end smoke tests (host+client), NAT simulation, fuzz tests for parsing.
- UX: better diagnostics and "why it failed" guidance with clear next actions.

