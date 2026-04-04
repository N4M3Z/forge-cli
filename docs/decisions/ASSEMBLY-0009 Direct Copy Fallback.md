---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
tags: [assembly, deployment, fallback]
---

# Direct Copy Fallback

## Context and Problem Statement

The assembly pipeline produces a `build/` directory with provider-specific output. Deployment copies these files to provider directories. While rulesync [1] or native CLI commands can handle deployment, a zero-dependency fallback must exist for environments where neither is available.

## Decision Drivers

- Users may not have Node.js (rulesync) or provider CLIs installed
- The deployment step is a flat file copy — no transformation needed
- A shell script or trivial binary covers the 4 core providers
- Direct copy to provider directories must always work

## Decision Outcome

Ship a minimal `forge copy` command that copies assembled output to provider directories. It reads provider config (prefix, extension) from `defaults.yaml` and copies files — no formatting, no transformation, no dependencies beyond a POSIX shell and a YAML reader.

```sh
forge install .         # assemble + copy (convenience wrapper)
forge assemble .        # assemble only → build/
forge copy build/       # copy only (build/ → provider dirs)
```

`forge copy` is deliberately named to signal that it does nothing smart — it copies files. If rulesync is available, users can run `rulesync generate` instead. Both produce the same result from the same `build/` input.

### Consequences

- [+] Zero external dependencies for basic deployment
- [+] `build/` is inspectable before deployment
- [+] rulesync, native CLIs, and `forge copy` all work interchangeably
- [-] `forge copy` only covers providers defined in defaults.yaml

## More Information

[1]: https://github.com/dyoshikawa/rulesync "rulesync — multi-provider AI tool config sync"
