---
title: "Embedded Assets via rust-embed"
description: "Optional compile-time asset embedding for standalone binary distribution"
type: adr
category: cli
tags:
    - cli
    - distribution
    - features
status: accepted
created: 2026-03-20
updated: 2026-03-20
author: "@N4M3Z"
project: forge-cli
related:
    - "RUST-0007 Feature Flags"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
upstream: []
---

# Embedded Assets via rust-embed

## Context and Problem Statement

forge-cli normally reads content from the filesystem (module repos, `defaults.yaml`). For standalone distribution — where a single binary must work without the source repo — content needs to be compiled into the binary. proton-agents uses rust-embed [1] for this: agent markdown, skill files, hooks, and rules are baked into the binary at compile time.

## Decision Drivers

- `cargo install` users may not have a module repo checked out
- Standalone distribution requires zero filesystem dependencies
- Embedded content goes stale when source files change — acceptable for versioned releases
- Not all users need embedded assets — most work from source repos

## Considered Options

1. **Always embed** — all content compiled into every build. Bloats development binary, stale during development.
2. **Optional feature flag** — embed only when building releases. Development reads from disk.
3. **External packaging** — distribute tarballs alongside the binary. Two artifacts to manage.

## Decision Outcome

rust-embed is an optional feature. When enabled, the binary carries content internally. When disabled (default), everything reads from disk.

```toml
[features]
default = ["full"]
full = ["assemble", "validate"]
embed = ["dep:rust-embed"]
```

```rust
#[cfg(feature = "embed")]
#[derive(rust_embed::Embed)]
#[folder = "agents/"]
struct EmbeddedAgents;
```

The `embed` feature is opt-in. Development builds read from disk. The user-facing interface is a flag on `forge release`:

```sh
forge release .                # tarballs only (per provider)
forge release . --embed        # tarballs + standalone binary with content baked in
```

`--embed` triggers a `cargo build --features embed` under the hood, producing a binary that carries the module's content internally. That binary can then `forge install --embedded` on any machine without a source repo.

The assembly pipeline works identically in both modes — the only difference is where source bytes come from (filesystem vs compiled-in).

### Consequences

- [+] Single-binary distribution via `forge release --embed`
- [+] No impact on development workflow (feature disabled by default)
- [+] Same assembly pipeline regardless of content source
- [+] No separate `forge embed` command — just a flag on release
- [-] Embedded content is a frozen snapshot — stale until recompiled
- [-] Binary size grows with embedded content

## More Information

[1]: https://github.com/pyrossh/rust-embed "rust-embed — compile-time asset embedding for Rust"
