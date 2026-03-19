---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
tags: [rust, cargo, lints]
---

# Dependency and Lint Configuration

## Context and Problem Statement

Cargo.toml configuration affects compile-time safety, dependency hygiene, and code consistency. Several settings that catch bugs early are not enabled by default.

## Decision Outcome

### Cargo.toml lints

```toml
[lints.rust]
unsafe_code = "forbid"

[lints.rust.unexpected_cfgs]
level = "deny"
check-cfg = ["cfg(ci)"]

[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
```

### Dependency grouping

```toml
[dependencies]
# cli
clap = { version = "4", features = ["derive"] }

# serialization
serde = { version = "1", features = ["derive"] }
serde_yaml = "0.9"
```

### Optional dependencies

All optional dependencies use the `dep:` prefix [2] to prevent implicit feature creation:

```toml
[features]
testing = ["dep:tempfile", "dep:assert_cmd"]
```

### Dev self-reference

Tests activate features by self-referencing the crate [1]:

```toml
[dev-dependencies]
crate-name = { path = ".", features = ["testing"] }
```

### rustfmt.toml

```toml
group_imports = "One"
imports_granularity = "Module"
merge_derives = false
```

### Edition

Use Rust edition 2024 for new crates [3].

### Consequences

- [+] `unexpected_cfgs = "deny"` catches typos in cfg attributes [1]
- [+] `dep:` prefix prevents accidental feature creation [2]
- [+] Grouped dependencies are scannable at a glance [1]
- [+] Self-reference activates test features cleanly [1]

## More Information

[1]: https://github.com/nickel-org/nickel.rs "Proton muon Cargo.toml patterns (dependency grouping, dev self-reference, unexpected_cfgs)"
[2]: https://doc.rust-lang.org/cargo/reference/features.html#optional-dependencies "Cargo dep: prefix for optional dependencies, stable since Rust 1.60"
[3]: https://doc.rust-lang.org/edition-guide/rust-2024/ "Rust Edition 2024, shipped with Rust 1.85 (Feb 2025)"
