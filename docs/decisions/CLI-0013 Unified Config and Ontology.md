---
title: "Unified Config and Ontology"
description: "Forge uses one typed config file for ontology paths and extension roots"
type: adr
category: cli
tags:
    - cli
    - config
    - ontology
status: accepted
created: 2026-07-08
updated: 2026-07-08
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0011 Watchlist Monitored-Source Registry"
    - "RUST-0001 Structured Errors with ErrorKind"
    - "RUST-0006 Synchronous Core"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Unified Config and Ontology

## Context and Problem Statement

Forge commands need a shared vocabulary for user directories such as the workshop, archive, vault, data, mount, developer, documents, and git hooks locations. A shell-level `project.yaml` reader already carries part of that ontology, but Rust commands cannot rely on shell functions for child process execution, extension lookup, or machine-readable config output. The kernel also needs to stay small: config loading should provide facts to scripts without absorbing script-level capabilities into Rust.

## Decision Drivers

- One typed source of truth for ontology values
- Deterministic precedence for environment overrides, config files, and defaults
- Compatibility with the legacy `project.yaml` shape for one migration window
- Pure library logic with no command output side effects
- No async, network, or dynamic config dependencies

## Considered Options

1. **Keep `project.yaml` as the only config.** This avoids migration, but keeps the Rust CLI coupled to a file shaped for shell functions and leaves no top-level home for extensions, launch, and watch sections.
2. **Adopt `~/.config/forge/config.yaml`.** A typed user config can group ontology and extension roots while preserving deterministic environment override behavior.
3. **Use per-command config files.** Each feature could own a file, but scripts and external dispatch need the same ontology and would duplicate precedence rules.

## Decision Outcome

Chosen option: **Option 2**, because a single typed config gives the minimal Rust kernel enough shared facts to launch scripts and expose the resolved ontology while keeping behavior outside Rust.

Forge reads `~/.config/forge/config.yaml` into typed structs. The top-level config denies unknown fields so mistakes fail loudly, while reserved `launch` and `watch` sections remain parse-tolerant for future use. Ontology accessors resolve values with `FORGE_*` environment variables first, config file values second, and built-in defaults last. Path-like ontology values expand a leading `~/` after resolution.

If `config.yaml` is absent, Forge reads `project.yaml` from the same directory as a deprecated fallback and maps `defaults.domain` to the ontology `domain` key. The fallback emits one process-wide warning so interactive commands make the migration visible without spamming repeated loads.

## Consequences

- `forge config` can display the exact effective ontology, including provenance for each key.
- `forge exec` and external dispatch inject the same `FORGE_*` values into child processes.
- Legacy `project.yaml` users retain one release of overlap.
- Unknown top-level keys in `config.yaml` are errors, which prevents silent misspellings.
- Capability remains in scripts; Rust only loads config and reports resolved facts.

## More Information

- [CLI-0011 Watchlist Monitored-Source Registry](CLI-0011%20Watchlist%20Monitored-Source%20Registry.md)
- [RUST-0006 Synchronous Core](RUST-0006%20Synchronous%20Core.md)
