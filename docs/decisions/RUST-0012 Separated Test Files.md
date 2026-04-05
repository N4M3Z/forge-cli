---
title: "Separated Test Files"
description: "Sibling tests.rs files instead of inline cfg(test) blocks for clean separation"
type: adr
category: rust
tags:
    - rust
    - testing
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0004 Test Infrastructure"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Separated Test Files

## Context and Problem Statement

Rust allows tests inline via `#[cfg(test)] mod tests { ... }` at the bottom of the source file. This mixes production code with test code in the same file, making both harder to read. Long test modules push production code out of view.

## Considered Options

1. **Inline cfg(test) blocks** — tests at the bottom of source files. Mixes production and test code.
2. **Sibling tests.rs files** — dedicated test files alongside production code. Clean separation.

## Decision Outcome

Every module uses directory form with a sibling test file:

```
src/<module>/
    mod.rs       # production code only
    tests.rs     # all tests for this module
```

`mod.rs` ends with:

```rust
#[cfg(test)]
mod tests;
```

Tests use external fixture files (per RUST-0004), not inline string literals. Fixture-heavy tests belong in `tests.rs`. The only acceptable exception: a single parameterized test in `mod.rs` that loops over a fixture directory.

```rust
// mod.rs — acceptable: one test that loops over fixtures
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn all_fixtures_parse_without_error() {
        for entry in fs::read_dir("tests/fixtures/input/agents").unwrap() {
            let content = fs::read_to_string(entry.unwrap().path()).unwrap();
            assert!(parse_frontmatter(&content).is_some());
        }
    }
}
```

Anything beyond a single fixture-loop test goes in `tests.rs`.

## Consequences

- [+] Production files contain only production code
- [+] Tests are always in the same predictable location
- [+] Fixture files are human-readable markdown, not escaped string literals
- [-] More files in the directory tree
