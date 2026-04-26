---
title: "Copy Provenance"
description: "SLSA provenance sidecars at target source level when copying content between forge modules"
type: adr
category: assembly
tags:
    - assembly
    - provenance
    - copy
status: accepted
created: 2026-04-10
updated: 2026-04-10
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0002 Provenance Tracking"
    - "ASSEMBLY-0009 Direct Copy Fallback"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Copy Provenance

## Context and Problem Statement

`forge copy` ([ASSEMBLY-0009](ASSEMBLY-0009 Direct Copy Fallback.md)) copies source files between modules with no provenance, no manifest tracking. Assembly provenance ([ASSEMBLY-0002](ASSEMBLY-0002 Provenance Tracking.md)) covers the source-to-deployed-output pipeline during `forge install`, but those sidecars land in gitignored deployment directories (`.claude/.provenance/`). When content is adopted from one module into another — especially with renaming (`SecretScan` → `SecretsScan`, `TheOpponent` → `DevilsAdvocate`) — there is no machine-verifiable record of lineage in version control.

`forge drift` only matches by filename. Renamed adoptions are invisible. Manual `upstream:` frontmatter markers have no SHA pinning and drift silently.

## Decision Drivers

- Copy provenance must be version-controlled (not gitignored like assembly provenance)
- Must reuse the existing SLSA statement format and generation code
- `forge drift` needs provenance-based resolution for renamed files

## Considered Options

1. **Extend `forge copy` with provenance sidecars** — write SLSA sidecars to the target module's source tree alongside copied files, using a `copy/v1` build type
2. **New `forge adopt` command** — dedicated command for cross-module adoption with auto-injected `upstream:` frontmatter and provenance
3. **Status quo** — hand-maintained `upstream:` frontmatter and `[upstream]:` ref links only

## Decision Outcome

Extend `forge copy` with provenance generation. Sidecars are written to `.provenance/` directories in the **target module's source tree**:

```
target-module/
    rules/
        KeepChangelog.md
        .provenance/
            KeepChangelog.yaml          ← copy provenance (tracked in git)
    .claude/
        rules/
            KeepChangelog.md            ← deployed by forge install
            .provenance/
                KeepChangelog.yaml      ← assembly provenance (gitignored)
```

Copy provenance uses `buildType: https://github.com/N4M3Z/forge-cli/copy/v1` to distinguish from `assemble/v1`. The `resolvedDependencies` URI is the source file's relative path; `externalParameters.source` is the source module's repository URI from `module.yaml`.

The copy command loads the source module's `module.yaml` to resolve its repository URI. If no `module.yaml` exists, provenance is skipped (preserving the zero-dependency fallback behavior from ASSEMBLY-0009).

## Consequences

- [+] Version-controlled provenance — travels with source files in git
- [+] SHA-pinned lineage — records exact content hash at time of copy
- [+] Enables `forge drift` to resolve renamed files via provenance sidecars
- [+] Reuses existing SLSA statement format and `manifest::generate_statement`
- [-] Two provenance layers to reason about (copy at source, assembly at deploy)
- [-] Copy provenance becomes stale when the target file is manually edited post-copy
