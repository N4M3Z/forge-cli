---
title: "Launch Middleware Chain"
description: "forge launch composes tool environments through ordered middleware and script extension points"
type: adr
category: cli
tags:
    - cli
    - launch
    - middleware
    - dispatch
    - config
status: accepted
created: 2026-07-09
updated: 2026-07-09
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0013 Unified Config and Ontology"
    - "CLI-0014 Exec Runtime Contract"
    - "CLI-0015 Git-Style External Command Dispatch"
    - "RUST-0006 Synchronous Core"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Launch Middleware Chain

## Context and Problem Statement

Coding-tool launch wrappers need to combine local proxies, observability, redaction, containers, and terminal session managers without turning Forge into a provider-specific process supervisor. A shell wrapper can cover one workstation's defaults, but it cannot provide typed config, testable plan composition, or a stable script extension contract for modules. Forge already has a minimal-kernel command-dispatch model, so launch behavior should reuse that shape instead of hard-coding every future environment transform in Rust.

## Decision Drivers

- Preserve scoped child-process environment changes without mutating Forge's own environment
- Keep middleware order explicit and deterministic
- Let modules and extensions add middleware without recompiling Forge
- Reuse configured extension directories and `PATH` search semantics
- Keep launch synchronous and directly return the child process exit code
- Make plan composition testable without spawning real coding tools or terminal wrappers

## Considered Options

1. **Keep launch behavior only in shell functions.** This preserves local flexibility, but leaves no typed config, no portable tests, and no extension contract.
2. **Add first-class Rust flags for every proxy and wrapper.** This is easy to document, but each new environment transform expands the kernel.
3. **Model launch as an ordered middleware chain with built-ins plus script middleware.** Rust owns plan composition, dry-runs, and stable built-ins; external scripts patch the plan through JSON.

## Decision Outcome

Chosen option: **Option 3**, because middleware chains keep launch policy composable while preserving Forge's minimal-kernel extension model.

`forge launch <tool>` resolves tool configuration from the unified config and builds a `LaunchPlan` containing child environment entries, an optional API `base_url`, command wrappers, and best-effort preflight steps. Middleware runs in the order named by `--with a,b,c`; conflicting `base_url` or environment keys warn and use the later value. `--tmux[=name]` appends the `tmux` middleware as the outer wrapper, so it encloses any earlier wrappers.

Built-in middleware covers `pxpipe`, `otel`, `presidio`, `squid`, `docker`, and `tmux`. Unknown middleware named `foo` resolves to `forge-launch-mw-foo` using the same module `commands/`, configured extension directories, and `PATH` search order as external forge verbs. Forge sends the current plan as JSON on stdin and merges the JSON patch from stdout. If no built-in or script exists, launch fails with a message listing the known middleware.

The child receives only the composed environment. A plan `base_url` is exported through the selected tool's configured base URL environment variable; the default `claude` mapping uses `ANTHROPIC_BASE_URL`. `--dry-run` prints the resolved plan and final argv without spawning preflight steps or the tool.

## Consequences

- Shell-only launch behavior can move into a tested native command without retiring local functions.
- New middleware can be delivered as scripts and config rather than Rust enum variants.
- Middleware authors get a small JSON patch contract instead of direct access to Forge internals.
- Wrappers that need shell command strings, such as `tmux new-session`, remain launch-kernel concerns.
- Best-effort preflight steps can warn without blocking direct tool launches when optional local services are unavailable.

## More Information

- [CLI-0013 Unified Config and Ontology](CLI-0013%20Unified%20Config%20and%20Ontology.md)
- [CLI-0014 Exec Runtime Contract](CLI-0014%20Exec%20Runtime%20Contract.md)
- [CLI-0015 Git-Style External Command Dispatch](CLI-0015%20Git-Style%20External%20Command%20Dispatch.md)
- [RUST-0006 Synchronous Core](RUST-0006%20Synchronous%20Core.md)
