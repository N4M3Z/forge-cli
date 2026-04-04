---
title: "Structured Operation Results"
description: "Typed result structs for all CLI operations with human and JSON output modes"
type: adr
category: cli
tags:
    - cli
    - ux
status: accepted
created: 2026-03-20
updated: 2026-03-20
author: "@N4M3Z"
project: forge-cli
related:
    - "CLI-0003 Conflict Resolution on Install"
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Structured Operation Results

## Context and Problem Statement

CLI operations (install, assemble, validate, copy) touch multiple files across multiple providers. Every operation needs to report what happened — what succeeded, what was skipped, and what failed — in a structured way that's both human-readable and machine-parseable. This applies everywhere, not just install.

## Considered Options

1. **Exit codes only** — 0 for success, non-zero for failure. No detail on what happened.
2. **Structured result types** — typed structs with per-provider breakdown and optional JSON output.

## Decision Outcome

Every operation returns a structured result, not just an exit code:

```rust
pub struct InstallResult {
    pub installed: Vec<DeployedFile>,
    pub skipped: Vec<SkippedFile>,
    pub errors: Vec<String>,
}

pub struct DeployedFile {
    pub source: String,
    pub target: String,
    pub provider: String,
}

pub struct SkippedFile {
    pub target: String,
    pub provider: String,
    pub reason: SkipReason,
}

pub enum SkipReason {
    UserModified,
    TargetMismatch,
    Unchanged,
}
```

CLI output:

```sh
forge install .
# claude: 12 agents, 8 skills, 5 rules
# gemini: 12 agents, 8 skills
# codex:  12 agents, 8 skills
# skipped: 2 (user-modified)
# errors:  0
```

`--json` flag for machine consumption. Per-provider breakdown by default.

### Consequences

- [+] Clear reporting of partial installs
- [+] Machine-parseable output for CI integration
- [+] Per-provider breakdown shows what landed where
