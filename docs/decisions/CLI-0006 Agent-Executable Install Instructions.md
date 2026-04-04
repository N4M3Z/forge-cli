---
status: Accepted
date: 2026-03-25
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil", "WebResearcher"]
informed: []
tags: [documentation, installation, convention]
---

# Agent-Executable Install Instructions

## Context and Problem Statement

There is no standardized way for LLMs to know how to install software. README.md is written for humans. CLAUDE.md and AGENTS.md carry behavioral instructions for agents already working inside a repo. Neither answers "how do I get this running from scratch and know it worked?"

The Mintlify install.md standard [1] solves this with structured, task-oriented markdown that agents execute autonomously, including explicit success criteria so the agent knows when installation is complete.

## Decision Drivers

- Install instructions should not pollute CLAUDE.md/AGENTS.md (behavioral) or README.md (human-oriented)
- Verification criteria must be explicit and measurable
- The format must be human-readable and reviewable before execution

## Decision Outcome

Every repo ships an `INSTALL.md` following the Mintlify install.md standard [1]. This replaces the PAI pattern of separate INSTALL.md + VERIFY.md files — DONE WHEN embeds verification, VERIFY.md is retired.

The template lives at `templates/INSTALL.md`.

Format (9 required elements):

1. **H1 Title** — lowercase, hyphenated identifier
2. **Blockquote Summary** — brief description of what the software does
3. **Conversational Opening** — "I want you to install [product] for me. Execute all the steps below autonomously."
4. **OBJECTIVE** — concise goal statement
5. **DONE WHEN** — specific, measurable success condition
6. **TODO Section** — markdown checkboxes (3-7 items) listing core tasks
7. **Detailed Steps** — sequential instructions with explicit commands, task language ("You need to...", "You must...")
8. **EXECUTE NOW Closing** — "EXECUTE NOW: Complete the above TODO list to achieve: [restate DONE WHEN]"
9. **llms.txt Reference** — optional link for additional context

Content rules from the spec:
- Include: all shell commands for the core workflow, verification commands, minimal working example
- Exclude: troubleshooting, optional features, GUI-only steps, alternative methods, lengthy explanations

### Consequences

- [+] Predictable location — agents look for `INSTALL.md` in every repo
- [+] DONE WHEN embeds verification — no separate VERIFY.md needed
- [+] Does not pollute CLAUDE.md, AGENTS.md, or README.md
- [+] EXECUTE NOW triggers autonomous agent execution
- [+] Follows an existing open standard rather than inventing a new convention
- [-] The standard was superseded by skill.md [2] after six days, long-term adoption unknown

## More Information

[1]: https://github.com/mintlify/install-md "Mintlify install.md — standard for LLM-executable installation"
[2]: https://www.mintlify.com/blog/install-md-standard-for-llm-executable-installation "Mintlify blog — install.md standard (deprecated in favor of skill.md)"
