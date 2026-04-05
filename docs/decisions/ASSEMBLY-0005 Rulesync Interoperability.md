---
title: "Rulesync Interoperability"
description: "Optional rulesync integration for multi-provider deployment with built-in fallback"
type: adr
category: assembly
tags:
    - assembly
    - deployment
    - interop
status: proposed
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related:
    - "ASSEMBLY-0008 Adopt Rulesync Deployment Conventions"
    - "ASSEMBLY-0009 Direct Copy Fallback"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["WebResearcher"]
informed: []
upstream: []
---

# Rulesync Interoperability

## Context and Problem Statement

rulesync (github.com/dyoshikawa/rulesync) is a mature multi-provider config sync tool (900+ stars, 200+ releases, 21+ providers) that routes rules, skills, agents, hooks, and commands to provider-specific directories. It uses JSONC config, frontmatter-based targeting, and a lockfile for reproducible installs from git repos. forge-cli assembles and validates content — a different concern. The two tools are complementary.

## Decision Drivers

- Deployment (file routing to provider directories) is a commodity — don't reinvent it
- Assembly (frontmatter stripping, variant merging, provenance) is our unique value
- rulesync's lockfile pattern solves distribution in a way our manifest doesn't
- Not all users will have rulesync installed — need a fallback

## Considered Options

1. **Require rulesync** — forge-cli outputs to `.rulesync/`, users run `rulesync generate`. Hard dependency on a Node.js tool.
2. **Ignore rulesync** — forge-cli does its own deployment. Duplicate effort, limited to 4 providers.
3. **Optional integration** — forge-cli assembles to `build/`, deploys with a minimal built-in deployer. If rulesync is present, can optionally output in rulesync-compatible format for broader provider coverage.

## Decision Outcome

Chosen option: **Optional integration**. forge-cli assembles content and deploys with its own minimal deployer (reads provider config, copies files). rulesync is not required. For teams that use rulesync, forge-cli can output assembled content in a format rulesync can consume.

What rulesync handles that we don't need to build:

- Routing to 21+ providers beyond our core 4
- Lockfile-based git package management
- Hook event name mapping across providers
- Simulated features for tools with limited native support

What we handle that rulesync doesn't:

- Frontmatter stripping and reference link removal
- Variant merging (base + overlay with append/prepend/replace)
- Provenance tracking (source → deployed SHA chain)
- Module structure validation
- Convention enforcement (naming, heading hierarchy, DCI)

We will adopt rulesync's deployment conventions (directory layouts, target naming, scope tiers) rather than inventing our own. This includes their target identifiers (`claudecode`, `geminicli`, `codexcli`), feature types (rules, skills, commands, subagents, hooks), and per-file `targets` frontmatter for selective deployment.

## Consequences

- [+] No hard dependency on Node.js tooling
- [+] Built-in deployer covers the 4 providers we target
- [+] rulesync integration available for broader provider coverage
- [-] Maintaining our own minimal deployer alongside rulesync compatibility
