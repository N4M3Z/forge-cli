---
title: "Code Style and Tooling"
description: "rustfmt, clippy pedantic, and naming conventions for maximally explicit code"
type: adr
category: rust
tags:
    - rust
    - style
    - tooling
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0011 Descriptive Generic Parameters"
    - "RUST-0010 Dependency and Lint Configuration"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Code Style and Tooling

## Context and Problem Statement

Consistent formatting and lint configuration across modules prevents style drift and reduces review friction. Code must be maximally explicit — long variable names, long method names, descriptive generic parameters (per RUST-0011). Well-written code does not need comments. If code requires a comment to explain what it does, the names are wrong.

## Considered Options

1. **Default rustfmt + no clippy** — minimal configuration, allows inconsistency.
2. **Custom rustfmt + clippy pedantic** — strict formatting and linting, catches more issues at compile time.

## Decision Outcome

### rustfmt.toml

```toml
group_imports = "One"
imports_granularity = "Module"
merge_derives = false
wrap_comments = true
```

### Cargo.toml Lints

```toml
[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "warn", priority = -1 }
module_name_repetitions = "allow"
must_use_candidate = "allow"
missing_errors_doc = "allow"
missing_panics_doc = "allow"
```

### Annotations

- `#[must_use]` on types and functions whose return values should never be silently dropped
- `#[forbid(unsafe_code)]` via Cargo.toml lints — no exceptions

### Naming

- Kebab-case for binary and crate names
- Snake_case for source files (Rust enforced)
- Methods returning `bool` start with `is_` or `has_`
- Full names over abbreviations

### Testing

- Sibling test files (`mod.rs` + `tests.rs`), not inline `#[cfg(test)]` blocks
- Doc tests with runnable examples on every public function
- `assert_cmd` for binary-level integration tests
- `proptest` for property-based testing (persistence disabled — no regression files at crate root)
