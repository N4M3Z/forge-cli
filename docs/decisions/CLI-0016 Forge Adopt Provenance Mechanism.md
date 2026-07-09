---
title: "Forge Adopt Provenance Mechanism"
description: "forge adopt fetches upstream artifacts synchronously and records adopt/v1 provenance"
type: adr
category: cli
tags:
    - cli
    - adopt
    - provenance
    - dependencies
status: accepted
created: 2026-07-09
updated: 2026-07-09
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0010 Copy Provenance"
    - "CLI-0011 Watchlist Monitored-Source Registry"
    - "RUST-0006 Synchronous Core"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Forge Adopt Provenance Mechanism

## Context and Problem Statement

Forge modules need a repeatable way to bring in useful upstream skills without losing the source URL, fetched digest, or local transform history. Manual adoption leaves review notes outside the repository and makes later drift checks dependent on memory. The command also needs to fit Forge's synchronous CLI architecture and the existing source-side `.provenance/` sidecar scan.

## Decision Drivers

- Preserve upstream attribution and digest pins in typed provenance sidecars
- Keep the command synchronous and available under the default `full` feature
- Avoid network access in unit tests
- Reject path traversal before writing into a module
- Use a small blocking HTTP client instead of adding an async runtime

## Considered Options

1. **Keep adoption as a manual skill workflow.** This keeps Rust smaller, but provenance depends on a human writing matching sidecars correctly.
2. **Use `reqwest`.** It is familiar and already appears transitively through `gix`, but direct use would add an async-first client surface or extra blocking feature choices.
3. **Use `ureq` v3 with rustls.** It provides a blocking API, follows redirects by configuration, and reuses the rustls ecosystem already present through the git stack.

## Decision Outcome

Chosen option: **Option 3**.

`forge adopt <url>` classifies anchored HTTPS, GitHub blob/raw, and hermetic `file://` fixture URLs. It fetches bytes through `ureq` for HTTPS, rejects non-UTF-8 bodies, applies the `align` transform, writes the artifact under the requested module, and records a source-side `.provenance/<stem>.yaml` sidecar. GitHub URLs must carry a full 40-hex commit in the URL before Forge records `externalParameters.upstream_commit`; plain HTTPS sources record an empty commit field.

The sidecar uses the existing manifest provenance types with `buildType: adopt/v1`, `externalParameters.upstream_url`, `externalParameters.transforms_applied: ["align"]`, the landed subject digest, and one `resolvedDependencies` entry named `upstream` containing the fetched-body digest. Unit tests inject fetched bytes directly, and an ignored smoke can cover a real hosted skill separately.

## Consequences

- Adoption becomes reproducible enough for source-side provenance verification and drift detection.
- `ureq` adds a direct dependency, but it avoids tokio in `full` and keeps HTTP behavior blocking.
- Plain HTTPS adoption pins content by digest but cannot independently prove a source commit.
- GitHub branch or tag URLs are rejected until Forge has a commit-resolution path that does not weaken the pin.
- The dashboard's private attribution model remains separate from the manifest provenance model.

## More Information

- [ASSEMBLY-0010 Copy Provenance](ASSEMBLY-0010%20Copy%20Provenance.md)
- [CLI-0011 Watchlist Monitored-Source Registry](CLI-0011%20Watchlist%20Monitored-Source%20Registry.md)
- [RUST-0006 Synchronous Core](RUST-0006%20Synchronous%20Core.md)
