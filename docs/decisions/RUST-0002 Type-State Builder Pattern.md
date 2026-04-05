---
title: "Type-State Builder Pattern"
description: "Compile-time enforcement of required builder steps via generic type parameters"
type: adr
category: rust
tags:
    - rust
    - patterns
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Type-State Builder Pattern

## Context and Problem Statement

Complex objects with required configuration steps are typically built with the builder pattern. Traditional builders use `Option<T>` fields and validate in `build()` — failing at runtime. Rust's type system can enforce required steps at compile time.

## Decision Drivers

- Required configuration must be enforced by the compiler, not runtime checks
- No `Option<T>` fields for required parameters
- Each builder step should be discoverable via autocomplete

## Considered Options

1. **Traditional builder with Option fields** — runtime validation in `build()`. Simple but errors only at runtime.
2. **Type-state generics** — compile-time enforcement via generic parameters. Errors at compile time.

## Decision Outcome

Use type-state generics where each builder method consumes `self` and returns a new type with an updated parameter:

```rust
pub struct Builder<Transport = (), Storage = ()> {
    transport: Transport,
    storage: Storage,
}

impl<AnyStorage> Builder<(), AnyStorage> {
    pub fn with_transport(self, t: Http) -> Builder<Http, AnyStorage> {
        Builder { transport: t, storage: self.storage }
    }
}

impl<AnyTransport> Builder<AnyTransport, ()> {
    pub fn with_storage(self, s: FileStore) -> Builder<AnyTransport, FileStore> {
        Builder { transport: self.transport, storage: s }
    }
}

impl Builder<Http, FileStore> {
    pub fn build(self) -> Client { /* only callable when both set */ }
}
```

Use this pattern for objects with 2+ required configuration steps. For simpler construction, plain `new()` with required parameters is sufficient.

## Consequences

- [+] Compiler prevents calling `.build()` before required steps
- [+] No runtime validation, no `Option<T>` fields
- [-] More verbose than traditional builders for simple cases
