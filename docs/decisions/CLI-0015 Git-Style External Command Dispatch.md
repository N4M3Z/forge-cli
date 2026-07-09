---
title: "Git-Style External Command Dispatch"
description: "Unknown forge verbs dispatch to forge-<verb> scripts before failing"
type: adr
category: cli
tags:
    - cli
    - dispatch
    - extensions
    - scripts
status: accepted
created: 2026-07-08
updated: 2026-07-08
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0013 Unified Config and Ontology"
    - "CLI-0014 Exec Runtime Contract"
    - "RUST-0006 Synchronous Core"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Git-Style External Command Dispatch

## Context and Problem Statement

Forge needs room for local verbs, module-specific workflows, and extension commands without adding a Rust enum variant for every new action. Git solves this by dispatching unknown verbs to `git-<verb>` executables. The same pattern fits Forge because the kernel only needs to provide config, process execution, and dispatch; real capability can live in scripts owned by modules or extensions.

## Decision Drivers

- Keep the Rust CLI small and stable
- Let modules and extensions add verbs without recompiling Forge
- Preserve clap handling for known subcommands
- Share ontology environment injection with `forge exec`
- Fail unknown commands with a clean message and deterministic exit code

## Considered Options

1. **Reject every unknown clap subcommand.** This is strict, but forces every verb into Rust.
2. **Use clap `external_subcommand` and spawn `forge-<verb>`.** Known commands still parse normally, while unknown commands become script dispatch.
3. **Make an explicit `forge run <verb>` namespace.** This avoids fallback behavior, but it makes extension commands feel second-class and diverges from Git-style muscle memory.

## Decision Outcome

Chosen option: **Option 2**, because `external_subcommand` gives Forge a script extension point while preserving strong parsing for built-in commands.

Unknown `forge <verb>` invocations resolve to `forge-<verb>` by searching `FORGE_ROOT/commands/`, then each configured extension directory, then `PATH`. The first match wins. Forge passes remaining arguments verbatim, inherits stdio, injects `FORGE_ROOT`, resolved ontology `FORGE_*` values, and `CI=1`, then returns the child process exit code.

If no script is found, Forge prints `error: unknown command 'forge <verb>' (no forge-<verb> script found)` and exits 2. There is no clap panic fallback for unknown verbs.

## Consequences

- New verbs can be shipped as scripts in modules, extensions, or user `PATH`.
- The Rust command enum remains focused on kernel services and stable built-ins.
- External commands receive the same ontology context as exec scripts.
- Search order makes module-local commands override extensions, and extensions override ambient `PATH`.
- Script authors own their dependencies and behavior; Rust only dispatches.

## More Information

- [CLI-0013 Unified Config and Ontology](CLI-0013%20Unified%20Config%20and%20Ontology.md)
- [CLI-0014 Exec Runtime Contract](CLI-0014%20Exec%20Runtime%20Contract.md)
