---
title: "Exec Runtime Contract"
description: "Forge executes skill-bundled scripts through a small synchronous runtime contract"
type: adr
category: cli
tags:
    - cli
    - exec
    - skills
    - scripts
status: accepted
created: 2026-07-08
updated: 2026-07-08
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0013 Unified Config and Ontology"
    - "CLI-0015 Git-Style External Command Dispatch"
    - "RUST-0006 Synchronous Core"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Exec Runtime Contract

## Context and Problem Statement

Skills often need small executable helpers for tasks such as data shaping, local file transforms, and provider-specific glue. Implementing those verbs in Rust would grow the kernel and make every capability a compile-time feature. Forge needs a stable way to run scripts bundled next to `SKILL.md` while providing enough context for scripts to behave consistently across modules and extensions.

## Decision Drivers

- Keep new capabilities in scripts, not Rust commands
- Reuse the existing skill directory convention of `skills/<name>/SKILL.md`
- Provide deterministic environment and stdin contracts for automation
- Support JSON validation without requiring scripts to link Rust code
- Stay synchronous and avoid venv or package-manager orchestration in Rust

## Considered Options

1. **Add one Rust subcommand per capability.** This is easy to type-check, but it makes the kernel responsible for feature growth.
2. **Execute scripts declared by skill frontmatter.** Skills carry their own helper scripts and Forge only resolves, launches, and validates them.
3. **Require every script to be directly executable.** This matches Unix habits, but it makes Windows and archive extraction behavior more fragile and duplicates interpreter selection.

## Decision Outcome

Chosen option: **Option 2**, because a frontmatter-declared script contract lets skills ship capability without expanding the Rust kernel.

`forge exec <skill>` locates a skill under `FORGE_ROOT/skills/<skill>` or configured extension skill roots. The script comes from `--script <name>` or from an `exec:` block in `SKILL.md` frontmatter with `script`, optional `runtime`, optional `inputSchema`, and optional `outputSchema` fields. Missing script declarations return exit code 3 with guidance to declare `exec:` or pass `--script`.

Runtime dispatch is a small table. `.py` uses `uv run`, `.sh` and `.bash` use `bash`, `.ts` uses `deno run`, and `.js`/`.mjs` use `node`. TypeScript chooses Deno because it is a single self-contained runtime for scripts and does not imply `node_modules` management in Rust.

Forge injects `FORGE_ROOT`, `FORGE_SKILL_DIR`, resolved ontology `FORGE_*` values, `CI=1`, and `INPUT_*` variables derived from the top-level JSON input object. In JSON mode, Forge captures child output and wraps it as `{ ok, exit_code, structured, stdout, stderr }`. When `outputSchema` is present, child stdout must parse as JSON and validate against the schema file relative to the skill directory.

## Consequences

- Skills can carry executable capability next to their instructions.
- Scripts do not need executable bits because Forge invokes them through interpreters.
- Rust does not create virtual environments, install packages, or fetch dependencies.
- JSON wrappers give automation a stable envelope while preserving raw stdout and stderr.
- Schema validation belongs to the kernel boundary because it is a contract check, not a capability implementation.

## More Information

- [CLI-0013 Unified Config and Ontology](CLI-0013%20Unified%20Config%20and%20Ontology.md)
- [RUST-0006 Synchronous Core](RUST-0006%20Synchronous%20Core.md)
