---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
tags: [rust, cargo, features]
---

# Feature Flags

## Context and Problem Statement

Optional functionality should be compile-time selectable. Cargo features control which code compiles, which dependencies are pulled, and what capabilities the binary ships with. A disciplined feature hierarchy prevents dependency bloat and enables minimal builds.

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

### Consequences

- [+] Minimal builds possible for embedded or constrained environments
- [+] Dependency graph is explicit and auditable
- [+] `dep:` prefix prevents accidental feature creation from dependency names
- [+] `unexpected_cfgs = "deny"` catches typos in `cfg()` attributes
