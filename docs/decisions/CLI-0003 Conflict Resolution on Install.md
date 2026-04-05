---
title: "Conflict Resolution on Install"
description: "Manifest-based detection of user-modified files with skip/prompt/force modes"
type: adr
category: cli
tags:
    - cli
    - deployment
    - ux
status: accepted
created: 2026-03-20
updated: 2026-03-20
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0003 Manifest-Based Deployment Tracking"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Conflict Resolution on Install

## Context and Problem Statement

When `forge install` deploys assembled content to provider directories, a previously deployed file may have been edited by the user. Silently overwriting loses their changes. Silently skipping leaves stale content. The manifest (ASSEMBLY-0003) tracks deployed file digests, making detection possible.

## Decision Drivers

- User modifications must not be silently destroyed
- Stale deployed files must not silently persist
- Non-interactive mode (CI, scripts) must have a deterministic default
- Interactive mode should offer choices

## Considered Options

1. **Always overwrite** — simple but destroys user modifications without warning.
2. **Always skip modified** — safe but leaves stale content with no way to force update.
3. **Detect and choose** — manifest comparison detects modifications, user picks action per mode.

## Decision Outcome

Compare the deployed file's current hash against the manifest's `deployed` digest. Three states:

| State      | Manifest digest matches file on disk? | Action                    |
| ---------- | ------------------------------------- | ------------------------- |
| Unchanged  | Yes                                   | Overwrite silently        |
| Modified   | No                                    | Prompt or skip            |
| New        | No manifest entry                     | Write                     |

Conflict detection runs across all target providers — a file might be unchanged in `.claude/` but modified in `.gemini/`. Each target is checked independently.

When reporting conflicts, print the provenance chain so the user can trace what produced the deployed file:

### Non-interactive (default, CI)

Skip modified files. Report them with provenance. Exit 0 (partial install is acceptable).

```sh
forge install .
# Installed 12 agents, 8 skills across claude, gemini, codex
# Skipped 2 (user-modified):
#   .claude/agents/SecurityArchitect.md
#     source: agents/SecurityArchitect.md (abc123)
#     variant: agents/claude/SecurityArchitect.md (def456)
#     deployed: 789ghi (manifest) → aaa111 (on disk, modified)
#   .gemini/skills/php-conventions/SKILL.md
#     source: skills/PhpConventions/SKILL.md (bbb222)
#     deployed: ccc333 (manifest) → ddd444 (on disk, modified)
```

### Interactive (`--interactive`)

Prompt per modified file: overwrite, skip, show diff, backup and overwrite.

### Force (`--force`)

Overwrite everything. No prompts, no skips.

## Consequences

- [+] User modifications never silently destroyed
- [+] Provenance trace on conflicts shows exactly what produced the deployed file
- [+] Per-target detection catches modifications in any provider directory
- [+] CI gets deterministic behavior (skip + report)
- [+] `--force` available for clean reinstalls
- [-] Partial installs possible — user must resolve skipped files manually
