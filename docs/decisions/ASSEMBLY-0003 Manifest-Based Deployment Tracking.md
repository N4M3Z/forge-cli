---
status: Accepted
date: 2026-03-23
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
tags: [assembly, manifest, deployment]
---

# Manifest-Based Deployment Tracking

## Context and Problem Statement

Installing skills, agents, and rules to provider directories is a multi-step process. Without tracking what was deployed, subsequent installs cannot detect user-modified files (edited post-install) or unchanged files (skip for performance).

## Decision Drivers

- Incremental installs — skip unchanged files
- User modification detection — distinguish "forge installed this" from "user edited this"
- Simple format — no spec overhead for what is fundamentally a hash lookup table

## Decision Outcome

The manifest is a **deployment record**, not a build artifact. It lives at the target as a `.manifest` dotfile — one per provider directory. Assembly does not produce it; copy creates it after deploying files.

```yaml
agents/GameMaster.md:
    sha256: 1e1a469e...
rules/PlayerAgency.md:
    sha256: 43844e52...
skills/SessionPrep/SKILL.md:
    sha256: b3c67018...
```

Each entry maps a deployed file path (relative to the provider directory) to the SHA-256 hash of the content that was deployed.

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

- [+] Simple format — just `{path: sha256}`, human-readable
- [+] Lives at the target — survives `build/` cleanup
- [+] Per-provider — each target directory tracks its own deployments
- [+] No spec overhead — this is not an attestation, just a cache
- [-] Manifest corruption means full reinstall (acceptable risk)
