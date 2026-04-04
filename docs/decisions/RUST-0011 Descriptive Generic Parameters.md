---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
tags: [rust, naming, readability]
---

# Descriptive Generic Parameters

## Context and Problem Statement

Rust convention uses single letters for generic parameters (`T`, `S`, `E`, `K`, `V`). This is a holdover from C++ templates and Java generics. In practice, `impl<S> Builder<(), S>` requires a comment to explain that `S` means "any Storage type." The code is not self-documenting.

## Decision Outcome

Use full descriptive names for generic type parameters. The compiler doesn't care about length. Humans do.

```rust
// Not this
impl<S> Builder<(), S> {
    pub fn with_transport(self, t: Http) -> Builder<Http, S> { ... }
}

// This
impl<AnyStorage> Builder<(), AnyStorage> {
    pub fn with_transport(self, t: Http) -> Builder<Http, AnyStorage> { ... }
}
```

The name should describe what the parameter represents, not abbreviate it:

| Instead of | Write            |
| ---------- | ---------------- |
| `T`        | `Item`, `Value`  |
| `S`        | `AnyStorage`     |
| `E`        | `ErrorType`      |
| `K, V`     | `Key, Value`     |
| `F`        | `Handler`, `Callback` |
| `R`        | `Response`       |

Standard trait bounds (`T: Display`, `T: Send + Sync`) are the exception — single letters are acceptable when the bound itself documents the intent.

### Consequences

- [+] Generic code reads without comments
- [+] No "what does S mean?" questions in review
- [+] Autocomplete shows meaningful parameter names

This is an instance of the broader principle in RUST-0003: if code requires a comment to explain what it does, the names are wrong.
