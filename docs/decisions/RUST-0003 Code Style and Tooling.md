---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
tags: [rust, style, tooling]
---

# Code Style and Tooling

## Context and Problem Statement

Consistent formatting and lint configuration across modules prevents style drift and reduces review friction. Code must be maximally explicit — long variable names, long method names, descriptive generic parameters (per RUST-0011). Well-written code does not need comments. If code requires a comment to explain what it does, the names are wrong.

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
