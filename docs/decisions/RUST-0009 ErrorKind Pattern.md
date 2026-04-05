---
title: "ErrorKind Pattern"
description: "Full implementation pattern for Error struct with non_exhaustive ErrorKind enum"
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
    - "RUST-0001 Structured Errors with ErrorKind"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# ErrorKind Pattern

## Context and Problem Statement

`Result<T, String>` is simple but prevents callers from branching on error category. As the codebase grows, some callers need to distinguish between parse errors (skip the file) and I/O errors (abort the operation). The standard library uses `std::io::ErrorKind` for exactly this pattern.

## Considered Options

1. **`Result<T, String>` everywhere** — simple but no programmatic error discrimination.
2. **`Error` struct with `ErrorKind`** — callers branch on kind, follows std::io::ErrorKind convention.

## Decision Outcome

Use a hand-written `Error` struct with a `Copy`-able `ErrorKind` enum for modules with multiple failure modes. Keep `Result<T, String>` for simple internal functions where the caller only prints or propagates.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorKind {
    Parse,
    Config,
    Io,
    Deploy,
}

pub struct Error {
    kind: ErrorKind,
    message: String,
}

impl Error {
    pub fn new(kind: ErrorKind, msg: impl Into<String>) -> Self {
        Self { kind, message: msg.into() }
    }

    pub fn kind(&self) -> ErrorKind { self.kind }
}
```

`#[non_exhaustive]` allows adding variants without breaking callers. No `thiserror`, no `anyhow` — zero dependencies for error handling.

### When to use which

| Pattern              | Use when                                          |
| -------------------- | ------------------------------------------------- |
| `Result<T, String>`  | Internal functions where caller only prints       |
| `Result<T, Error>`   | Public API boundaries where callers branch on kind |

## Consequences

- [+] Callers branch on `ErrorKind`, not error text
- [+] Follows `std::io::ErrorKind` convention
- [+] `#[non_exhaustive]` future-proofs variant additions
- [+] Zero dependencies
- [-] More boilerplate than `thiserror` for Display/Error impls
