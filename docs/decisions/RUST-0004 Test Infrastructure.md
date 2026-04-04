---
title: "Test Infrastructure"
description: "External fixture files and testing feature flag for shared test utilities"
type: adr
category: rust
tags:
    - rust
    - testing
    - muon
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0012 Separated Test Files"
    - "RUST-0007 Feature Flags"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Test Infrastructure

## Context and Problem Statement

Tests that validate markdown processing need readable test data. Inline string literals with escaped newlines (`"---\nname: Test\n---\n\nBody"`) are unreadable for markdown-heavy content. Test utilities (mock dispatchers, fixture loaders) need to be shared between unit and integration tests without shipping in release builds.

## Decision Drivers

- Test fixtures must be real markdown files readable by humans
- Test utilities must be available to both unit and integration tests
- Test-only code must not ship in release builds
- Muon's `testing` feature flag pattern is the established Rust approach

## Considered Options

1. **Inline string literals** — test data as escaped strings in Rust. Unreadable for markdown content.
2. **External fixture files with testing feature** — real markdown files loaded via `include_str!`, shared utilities behind feature flag.

## Decision Outcome

### External fixture files

Test data lives as real files in `tests/fixtures/`, loaded via `include_str!`:

```
tests/
    fixtures/
        input/          # source markdown, hook JSON payloads
        expected/       # golden output for snapshot comparison
        configs/        # YAML module configs, dispatch manifests
```

```rust
const AGENT_BASIC: &str = include_str!("fixtures/input/agent_basic.md");
const EXPECTED_DEPLOY: &str = include_str!("fixtures/expected/agent_deployed.md");

#[test]
fn deploy_agent_strips_frontmatter() {
    let result = assemble::strip_front(AGENT_BASIC);
    assert_eq!(result, EXPECTED_DEPLOY);
}
```

### `testing` feature flag

Test utilities live in `src/` behind `cfg(feature = "testing")`. Dev-dependencies self-activate:

```toml
[dev-dependencies]
forge-cli = { path = ".", features = ["testing"] }
```

### TestDispatcher

A `VecDeque`-backed mock for hook dispatch testing:

```rust
#[cfg(feature = "testing")]
pub struct TestDispatcher {
    responses: VecDeque<(String, i32)>,
}
```

### Consequences

- [+] Fixtures are readable markdown files, not escaped strings
- [+] Golden output enables snapshot-style testing with clear diffs
- [+] Test utilities shared without shipping in release
- [-] `include_str!` resolves at compile time — fixture path changes require recompilation
