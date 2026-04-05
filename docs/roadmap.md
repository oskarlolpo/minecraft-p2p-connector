# Transport Roadmap

## Immediate Goal

Stabilize the connector around a clean direct-first and fallback-capable architecture.

## Phase 1: Cleanup

- stop mixing experimental transport branches into one state machine
- keep the current app buildable
- document target transport strategy
- isolate UI concerns from networking concerns

## Phase 2: Process Split

- introduce a dedicated networking helper process
- move socket ownership out of the Tauri UI process
- define structured IPC events between UI and helper

## Phase 3: Transport Rebuild

- implement direct transport path
- implement relay-compatible free fallback path
- add real transport selection state
- remove fake optimistic connection states

## Phase 4: Russia-Grade Validation

- test Novosibirsk direct path
- test Kazan hostile NAT path
- test Belarus cross-border path
- compare transport choice, success rate and latency

## Phase 5: Release Hardening

- improve logs and diagnostics bundle export
- finalize installer and update path
- publish public-facing release notes and validation matrix

## Non-Negotiable Rules

- no “connected” UI before backend confirmation
- no release without logs in UTF-8
- no transport added without a defined failure reason and fallback behavior
