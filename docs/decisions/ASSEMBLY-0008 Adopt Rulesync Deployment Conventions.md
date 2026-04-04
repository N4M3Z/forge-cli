---
title: "Adopt Rulesync Deployment Conventions"
description: "Adopt rulesync directory layouts, target naming, and scope tiers for deployment"
type: adr
category: assembly
tags:
    - assembly
    - deployment
    - rulesync
status: accepted
created: 2026-03-19
updated: 2026-04-04
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0005 Rulesync Interoperability"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["WebResearcher"]
informed: []
upstream: []
---

# Adopt Rulesync Deployment Conventions

## Context and Problem Statement

rulesync [1] defines a mature set of deployment rules for 21+ AI coding providers — directory layouts, file naming, feature routing, and per-target configuration. Rather than inventing our own deployment conventions, we should adopt theirs and acknowledge the source.

## Decision Drivers

- rulesync has 900+ stars, 200+ releases, and 52 contributors — battle-tested conventions
- Reinventing deployment rules wastes effort on a solved problem
- Alignment with rulesync enables future interoperability
- Our assembly pipeline is the unique value — deployment rules are commodity

## Considered Options

1. **Invent own conventions** — define our own directory layouts and target naming. Full control but duplicates solved work.
2. **Adopt rulesync conventions** — use their layouts and naming, acknowledge the source. Enables interoperability.

## Decision Outcome

Adopt rulesync's deployment conventions for how files are placed into provider directories. Copy their target definitions, feature routing, and directory layout rules. Acknowledge the source explicitly.

What we adopt:
- Target naming (`claudecode`, `geminicli`, `codexcli`, `opencode`)
- Directory layouts per target (`.claude/skills/`, `.gemini/skills/`, etc.)
- Feature types (rules, skills, commands, subagents, hooks)
- Per-file `targets` frontmatter for selective deployment
- Scope tiers (project → user → global)

What we keep as our own:
- Content assembly (frontmatter stripping, variant merging, ref removal)
- Provenance tracking
- Manifest-based deployment tracking
- Validation

Examples of conventions adopted from rulesync [1]:

- Target names: `claudecode`, `geminicli`, `codexcli`, `opencode`, `cursor`, `copilot`
- Directory layouts: `.claude/rules/`, `.claude/skills/`, `.gemini/rules/`, `.codex/agents/`
- Feature types: rules, skills, commands, subagents, hooks, ignore, mcp
- Frontmatter routing: `targets: ["claudecode", "geminicli"]` in source files
- Scope tiers: project (`./<provider>/`) → user (`~/.<provider>/`) → global
- Config format: JSONC with `targets` and `features` arrays

### Consequences

- [+] No need to research or document provider directory layouts ourselves
- [+] Future rulesync interoperability is natural
- [+] Contributors familiar with rulesync feel at home
- [-] Dependency on rulesync's conventions evolving

## More Information

[1]: https://github.com/dyoshikawa/rulesync "rulesync — multi-provider AI tool config sync (MIT, 900+ stars)"
