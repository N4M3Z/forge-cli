---
title: "Synchronous Core"
description: "All core operations synchronous to avoid async infection and runtime dependencies"
type: adr
category: rust
tags:
    - rust
    - architecture
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

# Synchronous Core

## Context and Problem Statement

Rust's async/await propagates through call stacks — once a function is async, every caller must be async too (the "function coloring" problem). This adds runtime dependencies, `Send` bounds, and lifetime constraints with no throughput benefit for filesystem operations.

## Decision Drivers

- Config loading, module discovery, and path resolution have negligible latency
- Async adds complexity (runtime dependency, `Send` bounds) with no benefit for file operations
- Once async enters the core, it infects every function signature in the call chain
- Any contributor should be able to work on the code without async expertise

## Considered Options

1. **Async throughout** — tokio runtime, async file I/O. Maximum throughput potential but unnecessary complexity for filesystem operations.
2. **Synchronous core with async boundaries** — sync by default, async only at explicit I/O boundaries if network calls are needed.

## Decision Outcome

All core operations are synchronous. Async is only permitted at explicit I/O boundaries if network calls are ever needed.

Synchronous: config loading, module discovery, path resolution, content assembly, validation, manifest tracking, provenance generation.

Async only if needed: network fetches (remote registries), external tool invocation with timeout.

## Consequences

- [+] No async runtime dependency
- [+] No `Send` + `Sync` bounds on types
- [+] Simpler error handling
- [+] Lower contribution barrier
- [-] Network operations would need blocking I/O or a scoped async runtime at the boundary
