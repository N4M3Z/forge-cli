---
status: Accepted
date: 2026-03-20
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
tags: [cli, ux]
---

# Structured Operation Results

## Context and Problem Statement

CLI operations (install, assemble, validate, copy) touch multiple files across multiple providers. Every operation needs to report what happened — what succeeded, what was skipped, and what failed — in a structured way that's both human-readable and machine-parseable. This applies everywhere, not just install.

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
