---
title: "Deny Unexpected Cfgs"
description: "Compile-time rejection of misspelled or stale cfg attributes via Cargo.toml lints"
type: adr
category: rust
tags:
    - rust
    - lints
    - cargo
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0007 Feature Flags"
    - "RUST-0010 Dependency and Lint Configuration"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Deny Unexpected Cfgs

## Context and Problem Statement

Rust's `#[cfg()]` attributes silently compile away code when the condition is false. A typo like `#[cfg(feature = "tesitng")]` silently drops the gated code with no compiler warning. This is especially dangerous when combined with feature-gated test utilities — a misspelled feature name means tests silently disappear.

## Considered Options

1. **Default behavior** — unexpected cfgs produce a warning. Easy to miss in CI output.
2. **Deny unexpected cfgs** — compile error on unrecognized cfg values. Catches typos immediately.

## Decision Outcome

Set `unexpected_cfgs` to `deny` in Cargo.toml with an explicit allowlist:

```toml
[lints.rust.unexpected_cfgs]
level = "deny"
check-cfg = ["cfg(ci)"]
```

This catches stale or misspelled feature flags at compile time. Custom cfg values (like `ci` for CI-only tests) must be declared in `check-cfg`. Stable since Rust 1.80 (July 2024), adopted by muon and other production Rust projects.

### Consequences

- [+] Typos in `cfg()` attributes are compile errors, not silent omissions
- [+] Stale feature flags caught during migration
- [+] Explicit allowlist documents all custom cfg values in one place
