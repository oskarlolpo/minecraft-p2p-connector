# GitHub Repository Inventory And Cleanup Plan

## Current repos under `oskarlolpo`

From the current authenticated GitHub account inventory:

### Public

- `oskarlolpo/minecraft-p2p-connector`
- `oskarlolpo/AyuGramDesktop-v6.3.10-clean`
- `oskarlolpo/bedrockRinth`
- `oskarlolpo/project`
- `oskarlolpo/modifications`
- `oskarlolpo/modrinth-bedrock-integration`
- `oskarlolpo/cursor-ai`
- `oskarlolpo/oskarlolpo`

### Private

- `oskarlolpo/MCPE-parser`
- `oskarlolpo/nonamegram-source`
- `oskarlolpo/tiktok2`
- `oskarlolpo/tiktok`
- `oskarlolpo/voiseee`
- `oskarlolpo/hub`
- `oskarlolpo/oskarlolpo666`
- `oskarlolpo/oskarlolpo-`
- `oskarlolpo/cleaner`

## Immediate problem

Several repos still have empty or weak descriptions. That makes the profile look like a dump instead of a curated engineering portfolio.

## Recommended normalization order

### Tier 1: public repos that affect first impression

1. `oskarlolpo/oskarlolpo`
2. `oskarlolpo/minecraft-p2p-connector`
3. `oskarlolpo/bedrockRinth`
4. `oskarlolpo/modrinth-bedrock-integration`
5. `oskarlolpo/cursor-ai`
6. `oskarlolpo/project`

### Tier 2: product and fork repos

1. `oskarlolpo/AyuGramDesktop-v6.3.10-clean`
2. `oskarlolpo/modifications`

### Tier 3: private repos

These still need internal README hygiene, even if they are not public.

## What a “normal page” should mean

For each repo, a sane repository front page should include:

- 1-line purpose
- current status: active / prototype / archived / fork / experiment
- core stack
- how to run
- what is incomplete
- screenshot or architecture note if it is an app

## Suggested public descriptions

### `minecraft-p2p-connector`

`Rust + Tauri desktop app for Minecraft Java LAN tunneling with NAT traversal, relay fallback, and GitHub Actions Windows releases.`

### `bedrockRinth`

`Bedrock-focused tooling and experiments around launcher, modpack, or ecosystem integration for Minecraft Bedrock.`

### `project`

This name is too generic. Either rename it or define it clearly. Current recommendation:

`General application prototype repository. Needs renaming or a sharper scope statement.`

### `cursor-ai`

`Experiments, notes, and tooling built around Cursor AI workflows and developer automation.`

### `oskarlolpo`

`Profile repository for project index, focus areas, and active engineering experiments.`

## Recommended next GitHub pass

1. Fix descriptions for all public repos.
2. Create or rewrite README for each public repo.
3. Mark abandoned repos as archived or explicitly experimental.
4. Rename generic repositories like `project` if they now have a real identity.
5. Make the profile repo `oskarlolpo/oskarlolpo` the navigation hub to everything else.

## Constraint

I have the repo inventory, but not all source repositories are checked out locally in this workspace. That means I can do one of two things next:

- bulk-update GitHub descriptions immediately via CLI
- or clone/open each repo and write proper README files one by one

The second path is the correct one if you want actual high-quality repo front pages instead of placeholder text.
