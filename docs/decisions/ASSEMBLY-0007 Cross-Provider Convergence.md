---
title: "Cross-Provider Convergence"
description: "Design forge-cli as assembler/validator with explicitly transitional deployment layer"
type: adr
category: assembly
tags:
    - assembly
    - providers
    - strategy
status: accepted
created: 2026-03-19
updated: 2026-03-19
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

# Cross-Provider Convergence

## Context and Problem Statement

The AI coding tool ecosystem is converging on shared conventions. The Agentic AI Foundation (Linux Foundation, co-founded by Anthropic, OpenAI, Block, Dec 2025) governs the AGENTS.md standard and MCP. The Agent Skills spec at agentskills.io defines the canonical SKILL.md format, adopted by Claude Code and Gemini CLI. Codex and Gemini already read `.agents/skills/`. Claude Code has open issues requesting `.agents/` support but hasn't shipped it yet.

This convergence changes what forge-cli needs to do over time.

## Decision Drivers

- `.agents/skills/SKILL.md` is becoming the universal format
- Provider-specific formatting (TOML for Codex, kebab-case for Gemini) is transitional
- forge-cli's deployment layer should shrink as providers converge
- Assembly and validation are the durable capabilities

## Considered Options

1. **Full provider abstraction** — build a thick deployment layer covering every provider difference. Maximizes coverage but most code becomes dead weight as providers converge.
2. **Assembler + validator with transitional deployment** — invest in durable capabilities, treat deployment as temporary scaffolding.

## Decision Outcome

Design the forge tool as an **assembler and validator** whose deployment layer is explicitly transitional.

### What's durable

- Content assembly (frontmatter stripping, variant merging, reference link removal)
- Provenance tracking (source → deployed SHA chain)
- Module validation (JSON Schema, heading structure, DCI)
- Manifest tracking (incremental installs, orphan cleanup)

### What's transitional

- Per-provider directory routing (`.claude/`, `.gemini/`, `.codex/`, `.opencode/`), probably
- Name format conversion (PascalCase → kebab-case)
- Body format conversion (markdown → TOML)
- Tool name remapping (Read → read_file)

### Timeline expectation

Once Claude Code adopts `.agents/skills/`, the deployment layer for skills reduces to a single directory copy. Agent deployment may take longer to converge (no shared agent format yet). Rule deployment has no upstream standard and remains forge-specific.

## Consequences

- [+] Assembly and validation investment is permanent
- [+] Deployment code can be removed without architectural impact
- [+] Aligns with ecosystem direction rather than fighting it
- [-] Must maintain transitional deployment code until convergence completes
