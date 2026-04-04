---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["WebResearcher"]
informed: []
tags: [assembly, providers, strategy]
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

### Consequences

- [+] Assembly and validation investment is permanent
- [+] Deployment code can be removed without architectural impact
- [+] Aligns with ecosystem direction rather than fighting it
- [-] Must maintain transitional deployment code until convergence completes
