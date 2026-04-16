---
title: "Provenance Tracking"
description: "in-toto/SLSA provenance sidecars for tracking assembly source-to-output chain"
type: adr
category: assembly
tags:
    - assembly
    - provenance
    - tracking
status: accepted
created: 2026-03-23
updated: 2026-04-17
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0003 Manifest-Based Deployment Tracking"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil", "WebResearcher"]
informed: []
upstream: []
---

# Provenance Tracking

## Context and Problem Statement

When source files are transformed during assembly (frontmatter stripped, variants merged, refs removed), the deployed file no longer matches the source. Debugging "where did this deployed rule come from?" requires tracing the assembly chain. Standard provenance formats exist — in-toto/SLSA for supply chain attestation, SPDX for software bill of materials, W3C PROV for general provenance.

## Decision Drivers

- Must track source → built file with SHA digests for both
- Must record variant derivations (which overlay was applied, in which mode)
- Must be human-readable (YAML, not binary)
- No external tooling dependencies for generation or consumption

## Considered Options

1. **in-toto/SLSA attestation** — industry standard for supply chain provenance, JSON-native, cosign/Sigstore tooling
2. **SPDX 3.0 Build Profile** — SBOM-oriented, designed for component inventory and build provenance
3. **W3C PROV-inspired YAML** — custom format using PROV vocabulary

## Decision Outcome

Chosen option: **in-toto/SLSA v1.0**, serialized as YAML. in-toto is the industry standard for build provenance [1], purpose-built for tracking "these inputs were transformed into this output by this builder." SLSA builds on in-toto with a structured `buildDefinition` that captures resolved dependencies with per-file digests [2].

Provenance is a **build record** — it answers "what sources produced this built file?" Each assembled file in `build/` gets a `.yaml` sidecar containing the SLSA statement.

Sidecars deploy alongside content to per-directory `.provenance/` subdirectories at the target (e.g., `~/.claude/agents/.provenance/SystemArchitect.yaml`). The `.manifest` references each sidecar via its `provenance` field. `forge provenance` reads these to verify deployed integrity.

Source-level staleness detection reads provenance sidecars to compare recorded source hashes against current source files. Deployment-level staleness is handled separately by `.manifest` at the target (see ASSEMBLY-0003).

### Release tarballs

`forge release` reuses install, so `.provenance/` subdirectories ship inside release tarballs alongside `.manifest`. Extracted tarballs preserve the full source-to-output chain, enabling `forge provenance` to verify a tarball's contents without re-running assembly.

```yaml
_type: https://in-toto.io/Statement/v1
subject:
    - name: claude/rules/AgentTeams.md
      digest:
          sha256: def456...
predicateType: https://slsa.dev/provenance/v1
predicate:
    buildDefinition:
        buildType: https://forge-cli/assemble/v1
        resolvedDependencies:
            - uri: rules/AgentTeams.md
              digest:
                  sha256: abc123...
            - uri: rules/user/AgentTeams.md
              digest:
                  sha256: 789ghi...
    runDetails:
        builder:
            id: forge-cli
            version:
                forge: 0.1.0
        metadata:
            startedOn: "2026-03-23T10:00:00Z"
```

For standardized in-toto `.link` attestations, `in-toto-run` can wrap `forge assemble` as an observer without changing the assembly pipeline [3].

## Consequences

- [+] Industry standard — cosign/Sigstore tooling for verification
- [+] Compact — one self-contained statement per output file
- [+] Source hashes enable source-level staleness detection
- [+] YAML serialization consistent with ecosystem
- [+] Sidecars deploy to `.provenance/` subdirs — referenced by `.manifest`, used by `forge provenance`
- [-] in-toto envelope adds structural overhead vs flat hashes

## More Information

[1]: https://in-toto.io/ "in-toto — framework for securing software supply chains"
[2]: https://slsa.dev/spec/v1.0/provenance "SLSA Provenance v1.0 Specification"
[3]: https://github.com/in-toto/in-toto "in-toto CLI — in-toto-run wraps commands as an observer to produce .link attestations"
