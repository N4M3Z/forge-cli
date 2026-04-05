---
title: "Feature Flags"
description: "Layered Cargo feature hierarchy for compile-time capability selection"
type: adr
category: rust
tags:
    - rust
    - cargo
    - features
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0005 Deny Unexpected Cfgs"
    - "RUST-0004 Test Infrastructure"
    - "CLI-0005 Embedded Assets via rust-embed"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Feature Flags

## Context and Problem Statement

Optional functionality should be compile-time selectable. Cargo features control which code compiles, which dependencies are pulled, and what capabilities the binary ships with. A disciplined feature hierarchy prevents dependency bloat and enables minimal builds.

## Considered Options

1. **No features** — everything always compiled. Simple but no way to reduce binary size or dependencies.
2. **Layered feature hierarchy** — atomic, composable features with dep: prefix convention.

## Decision Outcome

Adopt a layered feature hierarchy from day one:

```toml
[features]
default = ["full"]
full = ["assemble", "validate", "deploy"]
assemble = []
validate = ["dep:jsonschema"]
deploy = []
testing = ["dep:tempfile", "dep:assert_cmd"]
```

### Conventions

- `default` pulls `full` — `cargo install` gets everything
- Individual features are atomic and composable
- Optional dependencies use `dep:` prefix to prevent implicit feature leakage
- `testing` gates test utilities that live in `src/` (per RUST-0004)
- Dangerous features are named explicitly: `dangerous-insecure-*`
- Dev-dependencies self-reference to activate all features:

```toml
[dev-dependencies]
crate-name = { path = ".", features = ["testing"] }
```

### Lint enforcement

```toml
[lints.rust.unexpected_cfgs]
level = "deny"
check-cfg = ["cfg(ci)"]
```

Catches stale or misspelled feature flags at compile time (per RUST-0005).

## Consequences

- [+] Minimal builds possible for embedded or constrained environments
- [+] Dependency graph is explicit and auditable
- [+] `dep:` prefix prevents accidental feature creation from dependency names
- [+] `unexpected_cfgs = "deny"` catches typos in `cfg()` attributes
