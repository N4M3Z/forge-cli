---
title: "Direct Copy Fallback"
description: "Zero-dependency forge copy command for basic deployment without external tools"
type: adr
category: assembly
tags:
    - assembly
    - deployment
    - fallback
status: accepted
created: 2026-03-19
updated: 2026-04-04
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0005 Rulesync Interoperability"
    - "ASSEMBLY-0004 Assembly and Deployment Pipeline"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Direct Copy Fallback

## Context and Problem Statement

The assembly pipeline produces a `build/` directory with provider-specific output. Deployment copies these files to provider directories. While rulesync [1] or native CLI commands can handle deployment, a zero-dependency fallback must exist for environments where neither is available.

## Decision Drivers

- Users may not have Node.js (rulesync) or provider CLIs installed
- The deployment step is a flat file copy — no transformation needed
- A shell script or trivial binary covers the 4 core providers
- Direct copy to provider directories must always work

## Considered Options

1. **Require rulesync** — mandatory Node.js dependency for deployment. Blocks users without Node.js.
2. **Built-in forge copy** — minimal file copy command reading provider config. Zero external dependencies.

## Decision Outcome

Two commands handle deployment:

- `forge deploy` copies assembled output from `build/` to provider directories with manifest tracking, provenance, and incremental install. This is the normal deployment path after `forge assemble`.
- `forge copy` copies source files directly to a target directory — no assembly, no transforms, no manifest. A raw fallback for environments where the full pipeline isn't needed.

```sh
forge install .                    # assemble + deploy (convenience wrapper)
forge assemble .                   # assemble only → build/
forge deploy .                     # deploy from build/ → provider dirs
forge copy . --target ~/project    # raw copy, no assembly or transforms
```

`forge copy` is deliberately named to signal that it does nothing smart — it copies source files as-is to a single target directory. `forge deploy` is the manifest-tracked deployment path.

### Consequences

- [+] Zero external dependencies for basic deployment
- [+] `build/` is inspectable before deployment
- [+] rulesync, native CLIs, and `forge copy` all work interchangeably
- [-] `forge copy` only covers providers defined in defaults.yaml

## More Information

[1]: https://github.com/dyoshikawa/rulesync "rulesync — multi-provider AI tool config sync"
