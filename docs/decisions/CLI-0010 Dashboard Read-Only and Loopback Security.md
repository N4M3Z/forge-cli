---
title: "Dashboard Read-Only and Loopback Security"
description: "The dashboard never mutates, serves loopback only behind a Host guard, and routes detail views by index not path"
type: adr
category: cli
tags:
    - cli
    - dashboard
    - security
status: accepted
created: 2026-06-04
updated: 2026-06-04
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0012 Forge Dashboard"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Dashboard Read-Only and Loopback Security

## Context and Problem Statement

The [CLI-0012 Forge Dashboard](CLI-0012%20Forge%20Dashboard.md) serves local artifact, provenance, settings, hooks, and config content over HTTP. A local web server that reads files and reflects them in a browser is a real attack surface: a malicious page the user visits can use DNS rebinding to reach a service on `127.0.0.1`, a path segment in a URL can become a directory-traversal component, and any write endpoint would let a forged request mutate deployments. The dashboard had to be designed so none of these are possible.

## Decision Drivers

- A browser must never be able to mutate deployments or any on-disk state
- A remote page must never be able to read local content via the dashboard
- A URL must never be able to escape into arbitrary files on disk

## Considered Options

1. **Loopback bind only.** Necessary but insufficient: DNS rebinding lets a malicious page resolve a hostname to `127.0.0.1` and reach the server from the victim's browser.
2. **Loopback bind plus a Host-header allowlist.** Browsers always send `Host`; rejecting non-loopback host names defeats DNS rebinding.
3. **Auth tokens or a login.** Heavier than warranted for a single-user local dev tool, and still needs the path and write protections.

## Decision Outcome

The dashboard is read-only and loopback-confined, with three independent protections.

- **No on-disk mutation, no network in handlers.** The dashboard never writes to disk; deploy-from-dashboard is deferred to a later, explicitly authorized effort. The only non-GET endpoint, `/refresh`, re-scans local state and is a `POST`, so a cross-origin `GET` (an `<img>` or link) cannot trigger it. Scanning resolves watchlist git entries from the local cache only and never clones or fetches inside a request handler, so no forged request can drive network I/O.
- **Loopback bind plus Host guard.** The server binds a loopback address, and a middleware rejects any request whose `Host` header is not a loopback name (`127.0.0.1`, `localhost`, `forge.localhost`, `::1`). This blocks DNS-rebinding access even when the socket is reachable.
- **Index-based detail routing.** Detail routes for config, settings, schemas, and hooks carry a numeric position into a server-derived, deterministically ordered list (for example `/config/{index}`, `/settings/{harness}/{index}`), never a filesystem path. A URL segment therefore can never become a path-traversal component. The list order is stable across requests (sorted), and the index is bounds-checked, returning 404 on a stale or out-of-range value. The few routes that do accept a path (the deployed-file viewer) canonicalize the path and assert it stays within the allowed provider directory before reading.
- **Allowlisted read surfaces and scheme-guarded links.** The settings and config views serve only an allowlisted set of filenames, never the full contents of `~/.config/forge` or local and MCP config files that may carry secrets. Outbound links built from a module's `repository` or a sidecar `source` are rendered only when they are `http(s)` URLs, so a `javascript:` value cannot become a clickable link.

## Consequences

- DNS rebinding is rejected by the Host guard, so a remote page cannot read local content.
- Detail routes carry positions, not paths, so arbitrary-file reads are structurally impossible; the one path-accepting route canonicalizes and confines the path.
- There are no on-disk writes, and the single `POST` (`/refresh`) only re-scans local state without network access, so a forged request cannot mutate state or drive fetches.
- The settings and config views are allowlisted, so secrets and private per-artifact config under `~/.config/forge` are not served.
- Index routes are sensitive to list order, mitigated by deterministic sorting and bounds-checking; a follow-up could promote the index to a stable key if the lists ever churn between requests.
- Deploy-from-dashboard is unavailable until a separate, explicitly authorized effort adds it.

## More Information

- [CLI-0012 Forge Dashboard](CLI-0012%20Forge%20Dashboard.md): the dashboard this posture governs.
