---
status: Accepted
date: 2026-03-21
responsible: ["@N4M3Z"]
accountable: ["@N4M3Z"]
consulted: []
informed: []
tags: [rust, documentation]
---

# Rustdoc Convention

## Context and Problem Statement

Rust has a built-in documentation system — `rustdoc`. Doc comments are standard markdown. `cargo doc` generates HTML documentation, `cargo test --doc` runs code examples as tests. This is the universal standard across the Rust ecosystem [1], used by every major crate including Proton's muon.

## Decision Outcome

Follow the Rust API Guidelines [2] for documentation:

### Item documentation (`///`)

Every public function, struct, and enum has a doc comment with:

1. Summary line (one sentence, what it does)
2. Extended description (when needed)
3. `# Examples` with a runnable code block
4. `# Errors` when the function can fail

### Module documentation (`//!`)

Every module file starts with a `//!` block describing the module's purpose.

### What NOT to document

- Private functions — doc comments are for the public API
- Obvious getters — `/// Returns the name` on `fn name()` adds nothing
- Implementation details — comments explain why, doc comments explain what

## More Information

[1]: https://doc.rust-lang.org/rustdoc/ "The rustdoc Book"
[2]: https://rust-lang.github.io/api-guidelines/documentation.html "Rust API Guidelines — Documentation"
