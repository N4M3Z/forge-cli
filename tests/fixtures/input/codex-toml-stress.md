---
name: TomlStressFixture
description: "Inert fixture exercising TOML round-trip escape paths."
model: gpt-5.4
---

# TomlStressFixture

Body content for serializer stress, not a runnable agent.

Round-trip targets:

- regex with quantifiers and character classes: `(alpha|beta|gamma|delta)\s*=\s*['"][^'"]{8,}`
- backslash sequences: `\path\to\file` and `\\double`
- escaped double quotes: `\"already quoted\"`
- bracketed text that mimics a TOML table header: `[injected_section]`
- triple-quote sequence inside body text: `"""`
- whitespace escapes: `\t`, `\n`, `\r`
- unicode escape sample: `A`
