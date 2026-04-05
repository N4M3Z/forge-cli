---
title: "Content Assembly Pipeline"
description: "Custom assembly stage for transforming source markdown into deployable content"
type: adr
category: assembly
tags:
    - assembly
    - architecture
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0004 Assembly and Deployment Pipeline"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil", "WebResearcher"]
informed: []
upstream: []
---

# Content Assembly Pipeline

## Context and Problem Statement

Skills, agents, and rules are authored as markdown with YAML frontmatter — the established format across LLM tooling communities [1] and PKM tools like Obsidian [2]. The assembly pipeline itself is language-agnostic: it processes text files with frontmatter, not Rust-specific artifacts. Before deployment, source files need processing: extraneous frontmatter stripping, reference link removal, variant merging, and provider-specific formatting.

## Decision Drivers

- Authors write one source of truth; deployment targets may differ per provider
- Frontmatter carries metadata for tooling but has no function for the target scaffolding — deploying it wastes tokens
- Reference-style links (`[1]: url`) provide provenance in source but waste tokens in deployed content where the AI never follows them
- Variant overrides (user/, provider/) must merge with the base via append/prepend/replace modes (per CORE-0018 [3])

## Considered Options

1. **No assembly — raw copy** — copy source files directly to provider directories. Simple but deploys frontmatter, ref links, and ignores variants.
2. **Existing tools** — chezmoi handles file transforms but not frontmatter-aware assembly. pandoc handles format conversion but not overlay merging. No off-the-shelf tool covers this pipeline [4].
3. **Custom assembly stage** — a dedicated transform step between source and deployment.

## Decision Outcome

Chosen option: **Custom assembly stage**, implemented as a single module. A pure function: `(source_content, variant_content, provider) → deployed_content`. No I/O, no filesystem writes. The deployment layer handles file placement.

Steps:

1. **Parse** — extract frontmatter values (name, description, targets, mode) without full YAML deserialization
2. **Resolve variant** — check qualifier directories (user/ > provider/model/ > provider/ > base) for overrides
3. **Merge** — combine base + variant body using the variant's `mode` field (append, prepend, replace)
4. **Strip** — remove frontmatter delimiters, H1 heading, and reference-style link definitions from the assembled body
5. **Format** — apply provider-specific output formatting (YAML frontmatter for Claude/Gemini/OpenCode, TOML for Codex)

## Consequences

- [+] Source files are clean markdown readable in any viewer
- [+] Provider differences are handled at assembly time, not authoring time
- [+] Variant merging lets teams customize without forking
- [-] Assembly step adds build complexity vs. raw file copying

## More Information

[1]: https://agentskills.io/specification "Agent Skills spec — markdown with YAML frontmatter"
[2]: https://help.obsidian.md/Editing+and+formatting/Properties "Obsidian Properties — YAML frontmatter"
[3]: https://github.com/N4M3Z/forge-core "CORE-0018 Qualifier Directories for Model Targeting"
[4]: https://www.chezmoi.io/ "chezmoi — closest analogue, but no frontmatter-aware assembly"
