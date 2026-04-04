/// Convert markdown with frontmatter to TOML configuration format.
///
/// Extracts `description` from frontmatter. The body (everything after
/// frontmatter) becomes the `instructions` field in a TOML multi-line string.
///
/// Input (markdown):
/// ```md
/// ---
/// description: A helper agent
/// ---
///
/// Do helpful things.
/// ```
///
/// Output (TOML):
/// ```toml
/// # source: Helper.md
/// description = "A helper agent"
/// instructions = """
/// Do helpful things.
/// """
/// ```
///
/// ```
/// # use commands::transform::markdown_to_toml;
/// let md = "---\ndescription: A helper agent\n---\n\nDo helpful things.";
/// let toml = markdown_to_toml("Helper.md", md).unwrap();
/// assert!(toml.contains("description = \"A helper agent\""));
/// assert!(toml.contains("Do helpful things."));
/// ```
pub fn markdown_to_toml(source_name: &str, content: &str) -> Result<String, String> {
    use std::fmt::Write;

    let description = crate::parse::frontmatter_value(content, "description").unwrap_or_default();
    let body = crate::parse::frontmatter_body(content);

    let mut output = String::new();
    writeln!(output, "# source: {source_name}").expect("writing to String");
    writeln!(
        output,
        "description = \"{}\"",
        escape_toml_string(&description)
    )
    .expect("writing to String");
    write!(output, "instructions = \"\"\"\n{}\"\"\"\n", body.trim()).expect("writing to String");

    Ok(output)
}

/// Escape double quotes and backslashes for TOML basic string values.
///
/// `"A \"quoted\" path\\to\\file"` becomes valid inside TOML double quotes.
fn escape_toml_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            _ => escaped.push(character),
        }
    }

    escaped
}
