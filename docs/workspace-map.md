# Workspace Map

## Current rule

`G:\minecraftjava` itself remains the live git root of `minecraft-p2p-connector`.

I did **not** try to move the active repository into a nested `p2p` folder, because that would break the current git root, release workflow, local build paths, and Tauri project layout.

## New layout

- `G:\minecraftjava\src`
- `G:\minecraftjava\src-tauri`
- `G:\minecraftjava\signaling-server`
- `G:\minecraftjava\docs`
- `G:\minecraftjava\third-party-projects`
- `G:\minecraftjava\compiled-projects`
- `G:\minecraftjava\mcpe-parser-project`

## Moved out of the main project root

### Third-party reference repos

- `boringtun`
- `coturn`
- `frp`
- `headscale`
- `innernet`
- `nebula`
- `netmaker`
- `ockam`
- `quinn`
- `rathole`
- `rust-libp2p`
- `rustdesk`
- `tailscale`
- `tauri`
- `ZeroTierOne`

All of them were moved to:

- `G:\minecraftjava\third-party-projects`

### Compiled reverse-engineering targets

- `voxel`
- `kurai`

They were moved to:

- `G:\minecraftjava\compiled-projects`

### MCPE parser side project

- `MCPE parser`

It was moved to:

- `G:\minecraftjava\mcpe-parser-project\MCPE parser`

## Why this layout is better

- The active P2P project root is no longer visually mixed with unrelated network stacks.
- Reverse-engineering targets are isolated from source repositories.
- External reference code is still local and searchable, but no longer clutters the working tree.
- `signaling-server` stayed in place because it is part of the tracked project.

## Next cleanup candidates

- Move `dist` out of versioned workspace if it is only a build artifact.
- Periodically prune `node_modules` if disk pressure matters.
- Rename `mcpe-parser-project\MCPE parser` to a normalized ASCII folder later if you want cleaner scripting paths.
