# Minecraft P2P Connector

Low-latency Minecraft Java Edition P2P LAN connector built with Tauri, Rust, QUIC and direct UDP hole punching.

## Components

- `src-tauri/`: Tauri desktop app and Rust networking core.
- `src/`: HTML, Tailwind CSS and JavaScript frontend.
- `signaling-server/`: lightweight Rust signaling server for room codes and UDP endpoint exchange.

## Local Run

### 1. Start the signaling server

```powershell
cd G:\minecraftjava\signaling-server
cargo run
```

Optional environment variables:

```powershell
$env:SIGNAL_WS_ADDR="0.0.0.0:9001"
$env:SIGNAL_UDP_ADDR="0.0.0.0:9002"
```

### 2. Start the desktop app in dev mode

```powershell
cd G:\minecraftjava
npm install
npm run tauri dev
```

Optional environment variables for the desktop app:

```powershell
$env:MC_SIGNAL_WS_URL="ws://127.0.0.1:9001/ws"
$env:MC_SIGNAL_UDP_ADDR="127.0.0.1:9002"
```

## Test Flow

### Host

1. Start the signaling server.
2. Launch the desktop app.
3. Click `Host Game`.
4. Share the room code.
5. Make sure the local Minecraft Java server is reachable on `127.0.0.1:25565`.

### Client

1. Start the signaling server or point the app to the remote one.
2. Launch the desktop app.
3. Click `Connect`.
4. Enter the room code.
5. Open Minecraft Java and connect to `localhost`.

## Production Build

```powershell
cd G:\minecraftjava
npm install
npm run build
cargo check --manifest-path src-tauri\Cargo.toml
```

GitHub Actions builds Windows `nsis` and `msi` bundles on tag push using `.github/workflows/release.yml`.

## Networking Notes

- The app uses the same UDP socket for signaling registration and QUIC transport to maximize NAT traversal success.
- This project intentionally has no relay fallback. Symmetric NAT can still fail without TURN or a relay.
