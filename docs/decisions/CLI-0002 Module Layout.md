---
title: "Module Layout"
description: "Library modules plus CLI handlers with separated test files"
type: adr
category: cli
tags:
    - rust
    - architecture
status: accepted
created: 2026-03-19
updated: 2026-04-04
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

Library modules plus CLI handlers:

```
src/
    commands.rs         lib crate root (re-exports all library modules)
    main.rs             binary entry point

    parse/              extract frontmatter values from markdown
    assemble/           transform source → deployable content (feature-gated)
    manifest/           SLSA/in-toto tracking, manifest read/write, provenance
    provider/           provider conventions, content kinds, config loading
    validate/           check files against .mdschema + YAML schemas (feature-gated)
    target/             resolve where to deploy (scope × provider × path)
    transform/          kebab-case, tool remapping, TOML conversion
    yaml/               YAML value extraction and deep merge
    module.rs           typed module.yaml deserialization
    result.rs           ActionResult, DeployedFile, SkippedFile types
    error.rs            ErrorKind + Error types

    cli/                CLI command handlers (binary-only, owns I/O)
        mod.rs          clap definitions, dispatch
        install.rs      assemble + deploy
        assemble/       assembly orchestration (sources, pipeline, provenance, output)
        deploy.rs       build/ → provider dirs with manifest tracking
        copy.rs         raw source → target (no assembly)
        validate/       structure + mdschema + external tools
        drift.rs        upstream comparison with frontmatter key diffing
        provenance/     provenance chain display and directory scanning
        release.rs      assemble + package tarballs
        config.rs       module config loading and merging
        output.rs       turbo-style CLI output formatting

tests/
    fixtures/
        input/          source markdown, configs
        expected/       golden output for snapshot comparison
        schemas/        YAML schema files for validation
```

### What each module does

| Module      | Input                        | Output                          |
| ----------- | ---------------------------- | ------------------------------- |
| `parse`     | markdown string              | frontmatter key-value pairs     |
| `assemble`  | source + variants + config   | stripped, merged, formatted body |
| `manifest`  | file paths + digests         | SLSA statement, manifest entries |
| `provider`  | defaults.yaml                | provider conventions + config   |
| `validate`  | file + .mdschema/YAML schema | pass/fail with diagnostics      |
| `target`    | scope + provider + kind      | resolved filesystem paths       |
| `transform` | content + rules              | kebab-case, tool remap, TOML    |
| `yaml`      | YAML string + key path       | extracted values, merged config |
| `module`    | module.yaml path             | typed ModuleManifest            |
| `cli`       | user args                    | I/O, exit codes, output         |

### Assembly details

`assemble` reduces frontmatter based on what the target provider expects. What stays (e.g., `name`, `description` for Claude Code skills [1]) is defined in config, not hardcoded. Everything else gets stripped. Variant resolution, ref link stripping, and provider-specific formatting are internal to `assemble`.

### Validation details

`validate` supports two schema formats:
- `.mdschema` — structural validation (headings, sections, required content) per CORE-0005
- YAML Schema — frontmatter field validation per ASSEMBLY-0006

### Targeting details

`target` resolves where assembled content lands. Given a scope (workspace/user/project), a provider, and a content kind (agents/skills/rules), it produces filesystem paths.

### CLI commands

| Command             | What it does                                                          |
| ------------------- | --------------------------------------------------------------------- |
| `forge install`     | assemble + deploy (daily workflow, all-in-one)                        |
| `forge assemble`    | source → `build/` (assembly only, inspect before deploy)              |
| `forge deploy`      | `build/` → provider dirs with manifest tracking and provenance        |
| `forge validate`    | structure + mdschema + external tools (shellcheck, cargo, tsc, gitleaks) |
| `forge drift`       | compare module against upstream, report frontmatter/body differences  |
| `forge provenance`  | show source chain for a deployed file or scan a directory             |
| `forge copy`        | raw source → target directory (no assembly, no transforms)            |
| `forge release`     | assemble + package as tarballs (+ optional `--embed`)                 |

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

- [+] Focused modules — each has one job
- [+] Each module testable with external fixtures
- [+] Library modules minimize I/O — CLI handlers own the boundary
- [+] No module does two things
