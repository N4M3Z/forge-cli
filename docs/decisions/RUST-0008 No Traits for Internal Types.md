---
status: Accepted
date: 2026-03-19
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: ["DeveloperCouncil"]
informed: []
tags: [rust, patterns]
---

# No Traits for Internal Types

## Context and Problem Statement

Traits in Rust scatter method implementations across files — the struct is defined in one place, trait impls in another. IDE "go to definition" may jump to the trait declaration instead of the implementation. For internal types with no polymorphism, this indirection makes code harder to navigate and understand.

## Decision Outcome

No traits for internal types. Use concrete structs with inherent methods. Every method is defined on the struct itself, in the same file, findable with a single "go to definition."

Traits are permitted only for:
- Standard library interop (`Display`, `FromStr`, `Error`, `Default`, `Serialize`)
- External crate requirements (clap `Parser`, serde `Deserialize`)
- Genuinely polymorphic boundaries where 2+ real implementations exist today

### Consequences

- [+] Every method lives on the struct — one place to look
- [+] No scattered impl blocks across files
- [+] No "where did this method come from?" confusion
- [+] Simpler refactoring — rename a method, find all callers directly
- [-] Adding polymorphism later requires extracting a trait and updating callers
