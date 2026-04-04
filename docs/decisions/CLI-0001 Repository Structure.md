---
title: "Repository Structure"
description: "Conventions and upstream references governing forge-cli repo layout"
type: adr
category: cli
tags:
    - architecture
    - structure
status: accepted
created: 2026-03-19
updated: 2026-03-19
author: "@N4M3Z"
project: forge-cli
related: []
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
upstream: []
---

# Repository Structure

## Context and Problem Statement

forge-cli is a Rust binary crate that assembles, validates, and deploys AI coding tool content (skills, agents, rules). It needs a clear structure that documents where things go and which upstream decisions apply.

## Considered Options

1. **Ad-hoc structure** — let the layout emerge organically. Risks inconsistency as the project grows.
2. **Documented conventions with upstream references** — explicitly reference CORE ADRs for each structural decision. One place to understand the repo.

## Decision Outcome

### Configuration

`config.yaml` (user, gitignored) > `defaults.yaml` (committed) > compiled `Default` impl. Per CORE-0007 [1].

### Content types

Skills, agents, and rules as markdown with YAML frontmatter. Per CORE-0008 [2].

### Validation

`.mdschema` files enforce frontmatter fields, heading hierarchy, and section structure on markdown files. Per CORE-0005 [3]. ADR schema deployed alongside decision records.

### Provenance

GFM reference-style links (`[1]: url`) in source files carry provenance. Stripped at deploy, preserved in `.yaml` sidecars alongside stripped frontmatter in `build/`. Per CORE-0017 [4] and CORE-0020 [5].

### Variant resolution

Qualifier directories (`user/` > `provider/model/` > `provider/` > base) for content overrides. Frontmatter `targets:` for include/exclude filtering. Merge modes: replace (default), append, prepend. Per CORE-0018 [6].

### Manifests

Dual SHA tracking (source + deployed) for incremental installs and orphan cleanup. Per CORE-0019 [7].

### Directory naming

Directories are navigation, not categorization. Every directory name is a routing decision. Qualifier directory names (`claude/`, `opus-4-6/`, `user/`) have functional consequences — a typo silently disables content. Per CORE-0024 [9].

### Licensing

EUPL-1.2. Per CORE-0015 [8].

### Consequences

- [+] Upstream decisions referenced, not duplicated
- [+] One place to understand the repo's conventions
- [+] New contributors read this ADR first

## More Information

[1]: https://github.com/N4M3Z/forge-core "CORE-0007 YAML Configuration with Deep Merge"
[2]: https://github.com/N4M3Z/forge-core "CORE-0008 Skills Agents and Rules"
[3]: https://github.com/N4M3Z/forge-core "CORE-0005 mdschema for Structural Validation"
[4]: https://github.com/N4M3Z/forge-core "CORE-0017 GFM Reference Links for Prompt Provenance"
[5]: https://github.com/N4M3Z/forge-core "CORE-0020 W3C PROV Provenance Records"
[6]: https://github.com/N4M3Z/forge-core "CORE-0018 Qualifier Directories for Model Targeting"
[7]: https://github.com/N4M3Z/forge-core "CORE-0019 Dual SHA Manifest"
[8]: https://github.com/N4M3Z/forge-core "CORE-0015 EUPL-1.2 Licensing"
[9]: https://github.com/N4M3Z/forge-core "CORE-0024 Directories Direct"
