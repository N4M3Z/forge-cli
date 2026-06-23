---
title: "Forge Dashboard"
description: "Read-only web dashboard for inspecting artifact state, provenance, and deployment across providers"
type: adr
category: cli
tags:
    - cli
    - dashboard
    - web
    - ux
status: accepted
created: 2026-06-04
updated: 2026-06-04
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0007 Interactive Mode and TUI"
    - "CLI-0005 Embedded Assets via rust-embed"
    - "RUST-0006 Synchronous Core"
    - "RUST-0007 Feature Flags"
    - "CLI-0010 Dashboard Read-Only and Loopback Security"
    - "CLI-0011 Watchlist Monitored-Source Registry"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Forge Dashboard

## Context and Problem Statement

The assembly and deployment pipeline produces a lot of state that is hard to see from the command line: which artifacts exist in each source repo, where they deploy, whether the deployed copy drifted from source, what the provenance chain looks like, and which references in a skill or rule no longer resolve. [CLI-0007 Interactive Mode and TUI](CLI-0007%20Interactive%20Mode%20and%20TUI.md) proposed a terminal UI for build inspection, manifest diffing, and provenance browsing, but deferred it. A visual surface was needed sooner, and a browser renders graphs, hyperlinks, and syntax-highlighted content far more cheaply than a terminal.

## Decision Drivers

- Visual inspection of artifacts, provenance graphs, manifest drift, and ADRs
- No new runtime weight in the default build
- A data model a future TUI can reuse rather than duplicate
- Fast iteration on layout without bespoke rendering code

## Considered Options

1. **Terminal UI (ratatui), per CLI-0007.** Rich and dependency-light at runtime, but provenance graphs, hyperlinks, and syntax highlighting are expensive to render in a terminal, and iteration is slow.
2. **Web dashboard (axum + htmx + Askama).** The browser handles rendering, links, and graphs; htmx keeps interactivity server-driven without a frontend build step. Adds an async web stack, which must be isolated.
3. **Static HTML export.** No server, but no live rescan, search, or interactivity.

## Decision Outcome

Ship a read-only web dashboard behind a `dashboard` cargo feature.

- **Stack:** axum 0.8 for routing, htmx (vendored, no CDN) for server-driven interactivity, Askama 0.13 for compile-time HTML templates. Static assets (CSS, vendored JS, highlight.js) are embedded with rust-embed, consistent with [CLI-0005 Embedded Assets via rust-embed](CLI-0005%20Embedded%20Assets%20via%20rust-embed.md).
- **Shared view model.** The data shapes live in the `commands` lib crate (`commands::view`), populated by the scanner, so a future TUI ([CLI-0007](CLI-0007%20Interactive%20Mode%20and%20TUI.md)) can render the same model instead of re-deriving it. The dashboard is the first consumer, not the owner, of these types.
- **Async is scoped to the web boundary.** tokio and axum are pulled only by the `dashboard` feature. The synchronous core ([RUST-0006 Synchronous Core](RUST-0006%20Synchronous%20Core.md)) is unchanged: the scanner and all pipeline logic stay synchronous, and async exists only in the request handlers. This is the "async only at explicit I/O boundaries" carve-out that RUST-0006 anticipated.
- **Feature composition.** The `dashboard` feature is off in the default build, so `cargo build` and the pre-commit checks never compile tokio, axum, or the dashboard module. It enables `full` (assemble, validate, deploy) because the dashboard reads the same source, provider, and provenance state the toolkit produces, so it builds as a self-contained superset rather than relying on the default set. A dedicated CI job compiles, lints, and tests `--features dashboard` so the feature cannot rot. Shared types used only by the dashboard carry `#[cfg_attr(not(feature = "dashboard"), allow(dead_code))]`. This follows [RUST-0007 Feature Flags](RUST-0007%20Feature%20Flags.md).

The dashboard surfaces an overview (artifacts grouped by kind and repository), repository detail, search, ADRs, a provenance graph (upstream to adopt to assemble to deploy), manifest drift, and read-only viewers for settings, hooks, config, and schemas.

## Consequences

- Deployment and provenance state is inspectable in a browser instead of only through CLI output.
- A future TUI renders `commands::view` rather than re-deriving the scan-to-model mapping.
- The default build and CI compile no async or web dependencies unless `dashboard` is enabled.
- The dashboard pulls an async web stack (axum, tokio), an exception to the synchronous core confined to the feature-gated request handlers.
- Askama compiles templates at build time, so template edits require a rebuild before they render.

## More Information

- [CLI-0007 Interactive Mode and TUI](CLI-0007%20Interactive%20Mode%20and%20TUI.md): the deferred TUI this dashboard renders over the web instead.
- [CLI-0010 Dashboard Read-Only and Loopback Security](CLI-0010%20Dashboard%20Read-Only%20and%20Loopback%20Security.md): the security posture of the server.
- [CLI-0011 Watchlist Monitored-Source Registry](CLI-0011%20Watchlist%20Monitored-Source%20Registry.md): how the dashboard knows what to scan.
