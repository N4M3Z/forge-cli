---
title: "Structured Errors with ErrorKind"
description: "Custom Error struct with Copy-able ErrorKind enum for programmatic error branching"
type: adr
category: rust
tags:
    - rust
    - error-handling
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0009 ErrorKind Pattern"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Structured Errors with ErrorKind

## Context and Problem Statement

`Result<T, String>` is the simplest error pattern in Rust — callers get a human-readable message but cannot programmatically distinguish error categories. The Rust community consensus [1] is `thiserror` for libraries, `anyhow` for applications. The standard library uses `std::io::ErrorKind` for structured error discrimination without external dependencies.

## Decision Drivers

- Callers should be able to branch on error category (skip vs abort vs retry)
- No unnecessary dependencies (no `anyhow`, no `thiserror`)
- ErrorKind must be cheaply comparable (`Copy` trait — can be passed by value without cloning)
- Source errors must be preserved for debugging

## Considered Options

1. **`Result<T, String>`** — current approach, simple, no branching possible
2. **`thiserror` derive macro** — adds a dependency, generates Display/Error impls
3. **Custom Error struct with ErrorKind enum** — zero dependencies, callers branch on kind, source chain preserved

## Decision Outcome

Chosen option: **Custom Error struct with ErrorKind enum**. For modules with multiple failure modes, use:

```rust
pub struct Error {
    kind: ErrorKind,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

// derive auto-generates trait implementations:
//   Debug  — enables {:?} formatting for logging
//   Clone  — allows copying the value
//   Copy   — makes copies implicit (no .clone() needed) because ErrorKind is small
//   PartialEq, Eq — enables == comparison between variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Parse,
    Config,
    Deploy,
}
```

`Result<T, String>` remains acceptable for simple functions where callers never branch on error kind. See RUST-0009 for the full implementation pattern with `#[non_exhaustive]` and factory methods.

## Consequences

- [+] Callers branch on `ErrorKind`, not error text
- [+] Source errors preserved via `Box<dyn Error>`
- [+] Zero external dependencies
- [-] More boilerplate than `thiserror` for Display/Error impls

## More Information

[1]: https://blog.rust-lang.org/2024/11/27/Rust-2024-survey-results.html "Rust Survey 2024 — error handling cited as top pain point"
