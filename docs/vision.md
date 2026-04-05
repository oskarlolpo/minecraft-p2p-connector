# Vision

## Project Name

Minecraft P2P Connector

## Mission

Build the best free Minecraft Java connector for Internet play that:

- does not require users to understand NAT or port forwarding
- does not force a VPN adapter as the default path
- works across difficult Russian and CIS networks
- keeps latency low enough for real gameplay

## Core Promise

The user experience must remain:

1. Open the app
2. Host or connect
3. Join Minecraft on `localhost:25565`

Everything else is implementation detail.

## What Success Means

The connector should be strong enough to be tested by players in:

- Novosibirsk
- Kazan
- Belarus

and still provide:

- reliable session establishment
- understandable failure modes
- low ping when a direct path is possible
- free fallback when a direct path is blocked

## Product Values

- clarity
- low-latency transport
- honest status reporting
- operational simplicity
- resilience under hostile consumer ISPs

## Product Strategy

We are not building:

- a generic VPN
- a full overlay network platform
- a toy NAT punching demo

We are building:

- a Minecraft-specific desktop connector
- optimized around localhost proxying
- transport-aware but user-simple

## Competitive Edge

The product must beat existing alternatives by combining:

- cleaner UX than technical tools
- lower friction than full VPN products
- more robust transport fallback than naive P2P-only tools
- better diagnostics than black-box launchers
