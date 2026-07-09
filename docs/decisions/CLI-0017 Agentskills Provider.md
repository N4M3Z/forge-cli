---
title: "Agentskills Provider"
description: "The Agent Skills provider uses the agentskills key and deploys to .agents"
type: adr
category: cli
tags:
    - cli
    - providers
    - agentskills
    - assembly
status: accepted
created: 2026-07-09
updated: 2026-07-09
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0011 Provider and Model Identifiers"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream:
    - "https://agentskills.io/specification"
---

# Agentskills Provider

## Context and Problem Statement

Agent Skills-compatible clients load skill directories from `.agents`, with each skill represented by a `SKILL.md` file and YAML frontmatter. Forge already has a generic provider deployment model where content lands at `<target>/<kind>/<relative>`, so `.agents` support should be provider data rather than another path-specific branch in deployment code. The provider name also participates in qualifier directory discovery for `rules/` and `agents/`, which creates a collision if the provider key is literally `agents`.

## Decision Drivers

- Add `.agents` deployment without provider-specific path code
- Preserve `forge install --provider agents` as a user-facing alias
- Avoid treating `rules/agents/` or `agents/agents/` as provider qualifier directories
- Keep Agent Skills frontmatter compatible with the published specification
- Avoid adding new assembly transforms

## Considered Options

1. **Name the provider `agents`.** This matches the target concept, but it makes `agents` a valid qualifier directory for non-skill content and can swallow ordinary nested content.
2. **Name the provider `agentskills` with alias `agents`.** This avoids qualifier collision while preserving the expected install selector.
3. **Special-case `.agents` in deployment code.** This works, but duplicates behavior already covered by provider `target` data.

## Decision Outcome

Chosen option: **Option 2**.

Forge defines provider key `agentskills`, target `.agents`, alias `agents`, and `assembly: [strip-links]`. The generic deployment path places a skill at `.agents/skills/<Name>/SKILL.md` with no path-code changes. Qualifier discovery sees `agentskills` as the provider name and never sees the alias, so `rules/agents/` is not a provider qualifier directory.

The Agent Skills specification requires `name` and `description`. It also recognizes top-level `license`, `compatibility`, `metadata`, and experimental `allowed-tools`; `version` is shown as `metadata.version`, not a top-level field. The provider keeps the recognized top-level fields and does not invent a top-level `version` field.

## Consequences

- Users can select the provider with either `agentskills` or `agents`.
- Existing provider assembly and deployment code handles the target path.
- The provider whitelist may need revision if the Agent Skills specification changes.
- Optional fields survive when authored, but unsupported Forge-specific fields are still stripped.

## More Information

- [Agent Skills specification](https://agentskills.io/specification)
- [ASSEMBLY-0011 Provider and Model Identifiers](ASSEMBLY-0011%20Provider%20and%20Model%20Identifiers.md)
