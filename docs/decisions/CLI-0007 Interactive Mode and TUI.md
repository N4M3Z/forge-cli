---
status: Proposed
date: 2026-03-26
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
tags: [cli, ux, future]
---

# Interactive Mode and TUI

## Context and Problem Statement

`forge install --force` overwrites user-modified files without confirmation. A middle ground between "skip everything modified" and "overwrite everything" requires per-file prompts. Beyond that, a terminal UI would make assembly inspection, manifest diffing, and provider selection visual.

## Decision Drivers

- Per-file confirmation before overwriting user-modified files
- Visual inspection of build/ output before deployment
- Provider selection (install to specific providers, not all)
- Manifest diff view (what changed since last install)

## Decision Outcome

Deferred to a future release. The `--interactive` flag was removed from v0.1.0 to avoid shipping dead code.

### Phase 1: Interactive prompts

`--interactive` on `install` and `copy` prompts before overwriting each Modified file. Uses stdin confirmation, no TUI dependency.

### Phase 2: TUI

A `forge tui` subcommand providing:
- Build output browser (tree view of build/ per provider)
- Manifest diff (deployed vs built, highlighting Stale/Modified)
- Provider picker (checkbox selection for which providers to deploy)
- Provenance inspector (sidecar viewer with source tracing)

Candidate crates: `ratatui` for terminal rendering, `crossterm` for input handling.

### Consequences

- [+] Per-file control without --force-or-nothing
- [+] Visual feedback for multi-provider deployments
- [-] TUI adds dependencies and complexity
- [-] Deferred — not available in v0.1.0
