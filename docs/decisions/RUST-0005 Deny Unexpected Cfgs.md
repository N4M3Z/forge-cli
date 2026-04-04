---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
tags: [rust, lints, cargo]
---

# Deny Unexpected Cfgs

## Context and Problem Statement

Rust's `#[cfg()]` attributes silently compile away code when the condition is false. A typo like `#[cfg(feature = "tesitng")]` silently drops the gated code with no compiler warning. This is especially dangerous when combined with feature-gated test utilities — a misspelled feature name means tests silently disappear.

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
