# Minecraft P2P Connector: Free Russia-Wide Architecture

## Why the previous direction failed

The `0.2.5` / `0.2.6` generation was closer to usable because it stayed small:

- direct tunnel
- simpler state model
- fewer moving parts

The later `libp2p + relay + bore` branch became too broad and unstable at once:

- too many fallback layers
- weak frontend/backend synchronization
- no clean transport ownership boundary
- fallbacks were added faster than observability

The result was not a reliable product.

## Hard constraints from the field

The user logs and ISP behavior imply a hostile environment:

- deep CGNAT on some Russian ISPs
- UDP STUN may be blocked entirely
- public relay TCP ports like `4001` are not reliable
- DNS may be broken at the ISP level
- some users are on Wi‑Fi and mobile-like last-mile quality

This means:

1. raw custom UDP hole punching is not enough
2. custom QUIC alone is not enough
3. direct connectivity must be attempted, but cannot be the only path
4. the fallback must look like ordinary Internet traffic

## Target architecture

We should build the next generation around:

1. `WebRTC DataChannel` as the primary transport
2. `WSS/443` signaling and ICE exchange
3. `TURN over TLS` compatibility path for hostile NAT/firewall cases
4. `Yggstack` / `Yggdrasil` as the no-TUN free mesh fallback for users who still cannot connect
5. separate helper process for networking, inspired by `kurai`

This is the only realistic free design that has a chance to cover most of Russia without forcing a TAP/TUN adapter into the main app.

## What to borrow from Kurai

`kurai` gets one architectural point right:

- the network tunnel is not the UI process

We should copy that pattern.

Recommended split:

- Tauri app
  - UI
  - settings
  - lobby
  - logs
  - process supervision
- helper binary
  - transport orchestration
  - ICE/WebRTC
  - fallback bridge
  - localhost proxy
  - telemetry/log stream to UI

This avoids:

- Tauri event spaghetti
- dead sockets after UI state changes
- mixed transport state in one process

## What to borrow from Voxel

`voxel` is useful mainly as a product reference:

- social-first lobby
- clearer server cards
- host/join flow that feels like matchmaking, not raw networking
- tighter separation between “public rooms” and “active session”

We should borrow:

- the lobby UX
- the idea that the transport is invisible to the player
- clearer connection state language

We should not borrow:

- random visual complexity
- anything that assumes media/video-first WebRTC UX

## Recommended external repos

### 1. `webrtc-rs/webrtc`

Use as the primary transport foundation.

Why:

- pure Rust
- includes STUN / TURN / ICE / DTLS / SCTP / DataChannel stack
- looks much more like normal Internet traffic than a custom game protocol
- far better chance of surviving hostile NATs than our raw QUIC punch path

Role in our app:

- direct peer connection
- TURN-compatible relay path
- reliable DataChannel for Minecraft TCP proxying

### 2. `yggdrasil-network/yggstack`

Use as the "last free fallback" concept.

Why:

- no TUN required
- explicit SOCKS5 and TCP port forwarder mode
- works as a standalone node
- can ride over ordinary transports supported by Yggdrasil, including `wss://`

Role in our app:

- universal fallback when direct ICE/WebRTC fails
- no virtual adapter in the main product flow
- lets us keep a localhost proxy model

### 3. `yggdrasil-network/yggdrasil-go`

Use as the underlying mesh reference for fallback mode.

Why:

- mature overlay design
- public peers ecosystem already exists
- not dependent on our own paid relay fleet from day one

Role in our app:

- community-backed fallback backbone
- regional public-peer routing

### 4. `libp2p/rust-libp2p`

Keep only for selective ideas, not as the primary transport anymore.

Why:

- good for peer identity, relay ideas, and protocol composition
- but in our case it became heavier than the result justified

Role in our app:

- optional future identity/discovery layer
- not the critical path for next rebuild

### 5. `ekzhang/bore`

Keep only as a debugging or emergency bridge tool.

Why:

- simple and useful
- but still TCP tunnel semantics
- not enough alone for a polished Russia-wide free connector

Role in our app:

- diagnostics
- developer fallback
- not the flagship production path

### 6. `rathole-org/rathole`

Keep as a self-hosted upgrade path, not the default free core.

Why:

- excellent reverse proxy for NAT traversal
- good if we later run our own relay edge

Role in our app:

- future optional relay tier
- not the zero-cost baseline

## Transport order

The next app should attempt connections in this order:

1. Direct WebRTC DataChannel with ICE
2. WebRTC with TURN/TLS compatible path
3. Yggstack/Yggdrasil fallback tunnel
4. Optional self-hosted relay tier later

Do not start with mesh fallback first.
Do not start with libp2p first.
Do not start with reverse TCP tunnel first.

## Proposed repo structure

The `p2p` repo should be reshaped around transport ownership:

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
├─ docs
└─ tools
```

### Responsibilities

`apps/tauri-ui`

- windows shell
- lobby
- settings
- translations
- logs

`apps/net-helper`

- one supervised sidecar executable
- owns sockets and transport state
- emits structured events to UI

`crates/bp-protocol`

- room metadata
- session state
- host/client commands
- event schema

`crates/bp-signaling`

- Ably today
- replaceable later

`crates/bp-webrtc`

- ICE
- STUN/TURN
- DataChannel transport

`crates/bp-yggstack`

- fallback launcher
- process integration
- port forwarding integration

`crates/bp-proxy`

- localhost:25565 handling
- host local-port bridge

`crates/bp-observability`

- UTF-8 logging only
- structured event codes
- exportable debug bundles

## Concrete next implementation steps

1. Freeze the current broken networking branch.
2. Recreate a clean branch from the last sane UX baseline (`0.2.5` / `0.2.6` logic).
3. Extract networking into a helper process.
4. Implement `bp-webrtc` first.
5. Implement event-driven UI state, no fake "connected" status.
6. Add `bp-yggstack` fallback only after WebRTC path is observable.

## What success looks like

For the user, the app should expose only:

- host room
- join room
- status: direct / relayed / mesh fallback
- one instruction: connect Minecraft to `localhost:25565`

For the developer, the app should expose:

- exact transport chosen
- ICE / TURN / fallback logs
- local server availability check
- reason why each previous transport was skipped

## Non-goals

- VPN adapter as the primary mode
- custom raw UDP protocol as the only path
- pretending to be connected before a real transport is live
- giant monolithic `main.rs` networking logic
