# Voxel And Kurai Analysis

## Scope

This is a practical reverse-engineering summary of two compiled projects found in the workspace:

- `G:\minecraftjava\compiled-projects\voxel`
- `G:\minecraftjava\compiled-projects\kurai`

The goal is not legal commentary. The goal is architectural extraction: what stack they use, what networking idea they rely on, and what is worth borrowing for `minecraft-p2p-connector`.

## Voxel

### What it is

`voxel` is a compiled **Flutter desktop app**.

Evidence:

- `flutter_windows.dll`
- `flutter_webrtc_plugin.dll`
- `libwebrtc.dll`
- `data\app.so`
- `data\flutter_assets\...`

### Functional clues from shipped assets

The asset set strongly suggests a Minecraft-oriented social/matchmaking shell:

- Minecraft font: `assets/font/Minecraft.ttf`
- P2P icon: `assets/p2p_40dp.png`
- Hoster icon: `assets/hoster.png`
- Minecraft icon: `assets/minecraft.png`
- language packs: `assets/lang/en.json`, `assets/lang/ru.json`

The bundled RU strings show these feature areas:

- social feed / posts
- subscribers / subscriptions
- multiplayer hosts list
- hoster mode
- Bedrock and Java sections
- passworded/private worlds
- in-app tutorial for joining worlds

### Networking takeaways

`voxel` ships `flutter_webrtc_plugin.dll` and `libwebrtc.dll`, which is the strongest clue that its realtime layer is based on **WebRTC data channels** rather than raw libp2p or QUIC.

That matters because:

- WebRTC is often better at surviving nasty consumer NATs than DIY UDP punching.
- WebRTC brings its own ICE/STUN/TURN ecosystem.
- It is heavier than your current Rust-native stack, but materially more battle-tested for hostile NAT traversal.

### Strategic value for your project

Useful ideas to borrow:

- hoster-mode UX
- friend/social discovery layer
- explicit private/public world distinction
- onboarding flow for Minecraft join instructions

Not worth copying directly:

- Flutter desktop runtime if your core target remains Rust + Tauri
- social feed complexity before the network path is stable

### Bottom line on Voxel

`voxel` looks like a **productized matchmaking shell** with WebRTC-era thinking and a social wrapper around Minecraft hosting. Its strongest lesson is not UI polish. Its strongest lesson is that **surviving bad NAT often requires transport camouflage and mature NAT traversal tooling**.

## Kurai

### What it is

`kurai` is a compiled **Electron app**.

Evidence:

- `rekurai.exe`
- `resources\app.asar`
- `LICENSE.electron.txt`
- Chromium/Electron runtime files

The `app.asar` top-level entries:

- `main.js`
- `preload.js`
- `dist`
- `package.json`

Extracted package info:

- app name: `rekurai`
- version: `0.1.62`
- main: `main.js`
- renderer stack hints: `framer-motion`, `emoji-picker-react`, `react-easy-crop`, `react-masonry-css`

### Real networking design

This one is more important than `voxel` for your current problem.

From extracted `main.js` and `preload.js`:

- Electron main process controls the desktop shell
- there is an updater executable: `resources\bin\updater.exe`
- there is a tunnel binary: `resources\bin\reku-tunnel.exe`
- there is also `wintun.dll`

The app spins a **local UDP socket in Node/Electron**, launches `reku-tunnel.exe`, gets a dynamically announced local port (`GOPORT:`), and relays bytes between:

- Electron/renderer WebRTC side
- local UDP tunnel process
- likely a virtual adapter / overlay path underneath

### Why Kurai matters

`kurai` is not pretending NAT traversal is easy. It uses:

- a dedicated helper binary for the tunnel
- a desktop shell only as orchestration/UI
- explicit updater and transport separation
- `wintun.dll`, which suggests a VPN-style overlay or virtual interface path

### Strategic value for your project

Useful ideas to borrow:

- strict separation of UI shell and tunnel engine
- dedicated helper process for networking instead of forcing everything into one event loop
- explicit lifecycle control of tunnel start/stop
- updater binary as an independent component

What conflicts with your stated target:

- `wintun.dll` means it is closer to overlay/VPN thinking
- this is exactly what you originally wanted to avoid

### Bottom line on Kurai

`kurai` is closer to a **hybrid overlay client** than a pure direct P2P connector. It is useful as an engineering reference for process isolation and tunnel lifecycle, but it does **not** validate the “no virtual adapter, pure direct P2P only” premise.

## Comparative conclusion

### Voxel teaches

- social discovery
- product UX
- probable WebRTC-based NAT strategy

### Kurai teaches

- hard separation between UI and transport engine
- updater/tunnel binary discipline
- willingness to use a stronger networking substrate when direct P2P is unreliable

## Recommendation for Blood Paradise Hub

If the goal stays:

- minimal latency
- no TAP/TUN
- Rust backend
- desktop UX in Tauri

then the clean next direction is:

1. Keep Tauri for UI.
2. Keep Rust for tunnel core.
3. Stop trying to make raw DIY direct-only NAT traversal the whole product.
4. Either:
   - move toward WebRTC-style traversal for the difficult cases, or
   - accept a hardened relay/reverse tunnel path as a first-class transport, not a shameful fallback.

Your logs already show the real market truth: the “final boss” ISPs are not edge cases anymore.
