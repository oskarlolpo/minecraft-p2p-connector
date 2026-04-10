# Minecraft P2P Connector

Windows desktop application for publishing and joining Minecraft Java Edition worlds with a layered connection strategy:

- direct QUIC / UDP hole punching for the fastest path
- relay fallback through Ably-backed coordination and reverse tunnel flow
- optional public fallback through e4mc, which exposes a public Minecraft join domain when direct connectivity is unavailable
- optional Bedrock bridge through bundled Geyser

The project is built with Tauri 2, Rust, vanilla HTML/CSS/JavaScript, Quinn, Ably, and a small embedded diagnostics toolset.

Current application version: `0.3.19`

## What The App Does

The host side publishes a local Java server or LAN world running on the machine. Other users discover it in the lobby and try to connect through the app. The app prefers the existing direct tunnel path and does not replace it.

If the normal path does not come up reliably, the host can also advertise a public `e4mc` domain. In that case players can still join the same Minecraft server through the public address backed by e4mc relay capacity.

This makes e4mc a fallback transport surface, not the primary architecture.

## Main Capabilities

- Host a local Java server or Open-to-LAN world
- Discover hosts through Ably Presence
- Attempt direct QUIC traversal first
- Keep the existing relay/tunnel behavior intact
- Expose a public e4mc join address as fallback
- Copy Java and Bedrock endpoints from the UI
- Add external Java servers manually to the list
- Run local diagnostics and a built-in test server
- Check GitHub releases and install updates from the app

## Connection Strategy

### 1. Primary path

The app first uses its existing networking stack:

- Ably for discovery, presence, and signaling
- QUIC and UDP hole punching for peer-to-peer connectivity
- existing reverse tunnel / relay logic when direct traversal needs help

### 2. Public fallback

When enabled in the host dialog, the app also starts an e4mc-backed host runtime:

- resolves the best e4mc relay through `broker.e4mc.link`
- establishes a QUIC control session using the `quiclime` ALPN
- requests a public domain assignment
- bridges inbound relay streams to the local Minecraft TCP port
- updates host presence with `public_join_address` and `e4mc_domain`

If a client cannot complete the normal app-based connection path, the UI copies the advertised e4mc address and prompts the user to join through standard Minecraft multiplayer using that public domain.

## Repository Layout

- `src-tauri/`: Rust backend, Tauri commands, networking core, updater, diagnostics
- `src/`: frontend UI, lobby, settings, host modal, translations
- `src-tauri/resources/geyser/`: bundled Geyser artifact used for Bedrock bridge mode
- `.github/workflows/release.yml`: release pipeline and version synchronization on tagged builds

## Hosting Modes

### Local host

Use this when the Minecraft server is running on the same machine:

- single-player world opened to LAN
- local Paper / Spigot / Fabric / Forge server
- local dedicated vanilla server

You provide the local Java port. The default is `25565`.

### External server list entry

The host modal can also add an external Java server into the lobby UI without running the local tunnel stack. This is only a convenience directory entry and does not create a P2P session.

## e4mc Fallback Behavior

The new fallback is intentionally constrained:

- it does not remove or replace the current tunnel implementation
- it only supplements host publishing with a public domain
- it depends on third-party infrastructure and should be treated as operationally best-effort

For product decisions this means:

- good fallback for users behind difficult NATs
- not a substitute for owned relay infrastructure
- protocol or policy changes on the e4mc side can affect availability

## UI And Versioning

The application version must stay synchronized in all three places:

- `package.json`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

If they drift, the settings screen and auto-update flow can report different versions and trigger false update behavior.

Version `0.3.19` updates these files consistently.

## Development

### Prerequisites

- Node.js 20+
- Rust stable
- Tauri prerequisites for Windows
- PowerShell

### Install

```powershell
cd G:\minecraftjava\p2p
npm install
```

### Run in development

```powershell
cd G:\minecraftjava\p2p
npm run tauri dev
```

This starts the Vite frontend and the Tauri desktop backend.

## Build

Frontend build:

```powershell
cd G:\minecraftjava\p2p
npm run build
```

Rust backend check:

```powershell
cd G:\minecraftjava\p2p
cargo check --manifest-path src-tauri\Cargo.toml
```

Production bundle:

```powershell
cd G:\minecraftjava\p2p
npm run tauri build
```

## Runtime Notes

### Minecraft Java

- Host a local world or server first
- If you open a single-player world to LAN, use the port Minecraft prints in chat/logs
- The app can try to auto-detect LAN port and nickname

### Bedrock / Geyser

If enabled, the app starts Geyser and publishes a Bedrock endpoint derived from the host address and configured Bedrock UDP port.

### Diagnostics

The diagnostics panel can:

- check whether a local Minecraft server is reachable
- start and stop an embedded test server
- probe the embedded test server directly
- copy logs and a diagnostics snapshot

## Update Flow

The settings page uses the backend app metadata and GitHub release checks.

Expected behavior:

- version shown in settings matches `CARGO_PKG_VERSION`
- GitHub releases are compared against the local app version
- install action should only appear when a newer release exists

If the UI shows an older version than the built binary, fix the three version files before publishing.

## Troubleshooting

### Host does not appear in the lobby

- confirm Ably connection is `connected`
- confirm the local Java server is reachable
- confirm the app has a public endpoint or e4mc domain to advertise

### Client cannot connect directly

- wait for the direct / relay path to finish negotiating
- if it fails and an e4mc address is present, use the copied public domain in Minecraft multiplayer

### Auto-update behaves incorrectly

- check `package.json`
- check `src-tauri/Cargo.toml`
- check `src-tauri/tauri.conf.json`
- ensure all three contain the same version

## Release Notes For 0.3.19

- added optional e4mc public fallback for host sessions
- presence now carries public join metadata
- UI can surface and copy e4mc join domains
- if direct in-app connection fails, the client now gets the public fallback address for manual join
- version mismatch fixed so settings and updater stop disagreeing about `0.3.17` vs `0.3.19`
