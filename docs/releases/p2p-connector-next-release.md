# GitHub Release Draft

## Title

Minecraft P2P Connector: architecture refresh for Russia-wide play

## Summary

This update prepares the connector for the next serious networking generation aimed at free, low-latency Minecraft play across hostile consumer networks.

The focus of this release is not cosmetic. It is a structural cleanup release that:

- locks in the new product direction
- documents the transport strategy
- aligns the repository with a cleaner long-term architecture
- prepares the codebase for a direct-first plus free-fallback networking rebuild

## Why this update matters

Previous experimental transport branches proved that a naive direct P2P strategy is not enough for users behind:

- CGNAT
- blocked UDP/STUN
- unstable Wi-Fi
- broken ISP DNS

This release establishes the blueprint for the next connector iteration that will be validated against real players in:

- Kazan
- Novosibirsk
- Belarus

## Included in this release

- refreshed repository vision and scope
- documented free Russia-wide networking architecture
- external product and transport reference analysis
- release and validation planning for the next major networking update

## Target outcome of the next networking generation

- users can host and join without white IPs
- no manual port forwarding
- no VPN adapter as the primary user experience
- Minecraft still connects to `localhost:25565`
- direct path is preferred for low ping
- fallback path remains free

## Validation goals

The next major transport update will be judged on:

- Kazan user behind hostile NAT
- Novosibirsk user direct path quality
- Belarus cross-region stability
- real in-game playability, not only handshake success

## Known reality

Low ping and free fallback can coexist only if:

- direct path is used whenever possible
- fallback is reserved for networks where direct transport is impossible

This project is being shaped around that rule.

## Repository documents included

- `docs/vision.md`
- `docs/roadmap.md`
- `docs/architecture/russia-free-connector.md`
- `docs/analysis/newrepo-13-projects-report.md`

## Next engineering step

Split the networking core into a dedicated helper process and rebuild the transport ladder around a more resilient direct-first architecture.
