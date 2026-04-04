---
title: "Module Layout"
description: "Six pure library modules plus CLI handlers with separated test files"
type: adr
category: cli
tags:
    - rust
    - architecture
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0012 Separated Test Files"
    - "RUST-0004 Test Infrastructure"
    - "ASSEMBLY-0006 Validation via YAML Schema"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Module Layout

## Context and Problem Statement

forge-cli assembles, validates, and deploys markdown content for AI coding tools. The codebase needs clear module boundaries where each module has one job, is easy to read, and has separated tests with external fixtures.

## Decision Drivers

- Each module has one job — readable in isolation
- No module exceeds ~300 lines of production code
- Every module uses directory form with sibling `tests.rs` (per RUST-0012)
- Library modules are pure (no I/O) — CLI handlers own the I/O boundary

## Considered Options

1. **Single module** — everything in one file. Simple for small projects but unreadable as it grows.
2. **Module-per-concern** — parse, assemble, manifest, provider, validate, target, cli. Each testable in isolation.

## Decision Outcome

Six library modules, one CLI module, one entry point:

```
src/
    lib.rs
    main.rs

    parse/              extract frontmatter values from markdown
        mod.rs
        tests.rs

    assemble/           transform source → deployable content
        mod.rs
        tests.rs

    manifest/           SLSA/in-toto tracking of inputs → outputs
        mod.rs
        tests.rs

    provider/           provider conventions + config loading
        mod.rs
        tests.rs

    validate/           check files against .mdschema + YAML schemas
        mod.rs
        tests.rs

    target/             resolve where to deploy (scope × provider × path)
        mod.rs
        tests.rs

    cli/                CLI command handlers (binary-only, owns I/O)
        mod.rs          clap definitions, dispatch
        install.rs      assemble + copy (daily workflow)
        assemble.rs     assemble only → build/
        copy.rs         build/ → provider dirs (or delegate to rulesync)
        validate.rs     check files against schemas
        release.rs      assemble + package tarballs (+ optional --embed)

tests/
    fixtures/
        input/          source markdown, configs
        expected/       golden output for snapshot comparison
        schemas/        YAML schema files for validation
```

### What each module does

| Module     | Input                      | Output                         | Pure |
| ---------- | -------------------------- | ------------------------------ | ---- |
| `parse`    | markdown string            | frontmatter key-value pairs    | yes  |
| `assemble` | source + variants + config | stripped, merged, formatted body | yes |
| `manifest` | file paths + digests       | SLSA statement (YAML)          | yes  |
| `provider` | defaults.yaml              | provider conventions + config  | yes  |
| `validate` | file + .mdschema/YAML schema | pass/fail with diagnostics   | yes  |
| `target`   | scope + provider + kind    | resolved filesystem paths      | yes  |
| `cli`      | user args                  | I/O, exit codes, output        | no   |

### Assembly details

`assemble` reduces frontmatter based on what the target provider expects. What stays (e.g., `name`, `description` for Claude Code skills [1]) is defined in config, not hardcoded. Everything else gets stripped. Variant resolution, ref link stripping, and provider-specific formatting are internal to `assemble`.

### Validation details

`validate` supports two schema formats:
- `.mdschema` — structural validation (headings, sections, required content) per CORE-0005
- YAML Schema — frontmatter field validation per ASSEMBLY-0006

### Targeting details

`target` resolves where assembled content lands. Given a scope (workspace/user/project), a provider, and a content kind (agents/skills/rules), it produces filesystem paths.

### CLI commands

| Command          | What it does                                             |
| ---------------- | -------------------------------------------------------- |
| `forge install`  | assemble + copy (daily workflow, all-in-one)             |
| `forge assemble` | source → `build/` (assembly only, inspect before deploy) |
| `forge copy`     | `build/` → provider dirs (or delegate to rulesync)       |
| `forge validate` | check files against schemas                              |
| `forge release`  | assemble + package as tarballs (+ optional `--embed`)    |

`forge copy` checks for rulesync on PATH. If present, delegates deployment. If not, copies files directly using provider config from `defaults.yaml`.

## More Information

[1]: https://docs.anthropic.com/en/docs/claude-code/skills "Claude Code skills — required frontmatter fields"

### Growth rule

If a module exceeds ~300 lines, split it into internal files within the module directory. If two modules always import each other, merge them. If a module has zero tests, it's probably doing too little — absorb it.

### Internal split pattern

When a module grows, add sibling files inside the module directory. `mod.rs` owns the public API and re-exports. Internal files use `pub(super)` — visible within the module, not exported to the crate.

```
assemble/
    mod.rs          pub API: assemble(source, variants, provider) → output
    strip.rs        strip_frontmatter, strip_refs
    merge.rs        resolve_variant, apply_variant (append/prepend/replace)
    format.rs       provider-specific output (yaml frontmatter, toml)
    tests.rs        tests for the public API
```

`mod.rs` declares internal files and re-exports:

```rust
mod strip;
mod merge;
mod format;

pub use strip::strip_frontmatter;
pub use merge::assemble;
```

Each internal file is focused — one concern, one file. The module boundary doesn't change; only the internal structure grows.

### Consequences

- [+] Five modules — fits in one mental model
- [+] Each module testable with external fixtures
- [+] Pure library — CLI handlers own all I/O
- [+] No module does two things
