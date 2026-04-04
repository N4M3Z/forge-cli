---
title: "Manifest-Based Deployment Tracking"
description: "SHA-256 manifest dotfiles at target directories for incremental install and modification detection"
type: adr
category: assembly
tags:
    - assembly
    - manifest
    - deployment
status: accepted
created: 2026-03-23
updated: 2026-04-04
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0002 Provenance Tracking"
    - "CLI-0003 Conflict Resolution on Install"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Manifest-Based Deployment Tracking

## Context and Problem Statement

Installing skills, agents, and rules to provider directories is a multi-step process. Without tracking what was deployed, subsequent installs cannot detect user-modified files (edited post-install) or unchanged files (skip for performance).

## Decision Drivers

- Incremental installs — skip unchanged files
- User modification detection — distinguish "forge installed this" from "user edited this"
- Simple format — no spec overhead for what is fundamentally a hash lookup table

## Considered Options

1. **No tracking** — always overwrite everything on install. Simple but destroys user modifications.
2. **Git-based tracking** — use git status in provider directories. Requires provider directories to be in a repo.
3. **Manifest dotfile** — per-provider `.manifest` with deployed file hashes. Simple, self-contained.

## Decision Outcome

The manifest is a **deployment record**, not a build artifact. It lives at the target as a `.manifest` dotfile — one per provider directory. Assembly does not produce it; copy creates it after deploying files.

```yaml
agents:
    GameMaster.md:
        fingerprint: 1e1a469e...
        provenance: agents/.provenance/GameMaster.yaml
rules:
    PlayerAgency.md:
        fingerprint: 43844e52...
        provenance: rules/.provenance/PlayerAgency.yaml
    cz:
        PersonalTaxIncome.md:
            fingerprint: b3c67018...
            provenance: rules/cz/.provenance/PersonalTaxIncome.yaml
```

The manifest is nested YAML mirroring the directory structure. Each leaf entry has a `fingerprint` (SHA-256 of deployed content) and a `provenance` pointer to the SLSA sidecar that records the source chain.

### Staleness detection

On subsequent installs, copy reads `.manifest` from the target and compares:

| Target hash vs `.manifest` | Build hash vs `.manifest` | Status    | Action              |
| -------------------------- | ------------------------- | --------- | ------------------- |
| matches                    | matches                   | Unchanged | skip                |
| matches                    | differs                   | Stale     | copy (safe)         |
| differs                    | —                         | Modified  | skip (or `--force`) |
| not in `.manifest`         | —                         | New       | copy                |

Source-level staleness (has the source changed since last build?) is detected by comparing provenance sidecars against current source files. See ASSEMBLY-0002.

### Consequences

- [+] Simple format — nested YAML with `fingerprint` and `provenance`, human-readable
- [+] Lives at the target — survives `build/` cleanup
- [+] Per-provider — each target directory tracks its own deployments
- [+] No spec overhead — this is not an attestation, just a cache
- [-] Manifest corruption means full reinstall (acceptable risk)
