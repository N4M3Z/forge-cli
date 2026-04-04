---
title: "Validation Script Distribution"
description: "Module validation via local script copy with upstream drift detection"
type: adr
category: cli
tags:
    - cli
    - validation
    - ci
status: accepted
created: 2026-04-02
updated: 2026-04-02
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0006 Agent-Executable Install Instructions"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil", "WebResearcher"]
informed: []
upstream: []
---

# Validation Script Distribution

## Context and Problem Statement

forge-cli validates modules via `forge validate`. CI and pre-commit hooks need the same validation without compiling the Rust binary. Curling a script at runtime from GitHub is a security risk — a compromised upstream silently changes what every module's CI executes. Hardcoding validation checks in each module's Makefile or CI YAML leads to drift and duplication.

## Decision Drivers

- CI must run full validation (shellcheck, clippy, ruff, tsc, ADR frontmatter) without the forge-cli binary
- No blind execution of remote code — `curl | bash` is not acceptable for CI or pre-commit
- Validation logic must not drift silently between modules and upstream
- The tool defines the spec, not the project — consistent with cargo, npm, pip patterns

## Considered Options

1. **CI compiles forge-cli** — `cargo install` in CI, then `forge validate .`. Slow (~30s compile), requires Rust toolchain
2. **curl | bash at runtime** — CI curls `validate.sh` from forge-cli repo. Fast but blind trust in upstream
3. **Local copy with drift detection** — each module commits a copy of `validate.sh`. CI runs the local copy. Drift check warns when upstream differs

## Decision Outcome

Chosen option: **local copy with drift detection**, because it combines security (no remote code execution) with maintainability (warnings when local copy is stale).

**forge-cli ships `bin/validate.sh`** — a standalone validation script that auto-detects module content and runs appropriate checks. Each module keeps a committed copy.

**CI template** at `templates/ci.yaml` sets up toolchains (`hashFiles` conditions for Rust, Python, Node) and runs the local `bin/validate.sh`.

**Pre-commit hook** tries `forge validate` (compiled binary, fastest), falls back to local `bin/validate.sh`.

**Drift detection** — on every run, `validate.sh` hashes itself against the upstream version at `github.com/N4M3Z/forge-cli/main/bin/validate.sh`. Mismatch emits a warning. Warning is informational, not blocking.

### Consequences

- [+] No remote code execution in CI or pre-commit
- [+] Drift detection catches stale copies without blocking
- [+] Local copy is auditable and version-controlled
- [+] CI and local dev run identical checks
- [-] Modules must manually update their copy (mitigated by drift warning)

## More Information

- [CLI-0006 Agent-Executable Install Instructions](CLI-0006 Agent-Executable Install Instructions.md) — INSTALL.md standard for module setup
