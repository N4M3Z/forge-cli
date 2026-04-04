use crate::yaml;

const MAX_CONTENT_SIZE: usize = 256 * 1024;
const FENCE: &str = "---";

/// Split markdown content into frontmatter YAML and body text.
///
/// Given this file:
///
/// ```md
/// ---
/// name: TestAgent
/// version: 0.1.0
/// ---
///
/// # TestAgent
///
/// Body content here.
/// ```
///
/// `split_frontmatter(content)` returns:
///   - `yaml_text` = `"name: TestAgent\nversion: 0.1.0"`
///   - body = `"\n# TestAgent\n\nBody content here."`
///
/// Returns `None` when:
///   - no frontmatter fence at start
///   - content exceeds 256 KB
///   - opening fence is never closed
pub fn split_frontmatter(content: &str) -> Option<(&str, &str)> {
    if content.len() > MAX_CONTENT_SIZE || !content.starts_with(FENCE) {
        return None;
    }

    let after_opening = content[FENCE.len()..]
        .strip_prefix('\n')
        .unwrap_or(&content[FENCE.len()..]);

    // Empty frontmatter: two fences back to back.
    if let Some(remainder) = after_opening.strip_prefix(FENCE) {
        let body = remainder.strip_prefix('\n').unwrap_or(remainder);
        return Some(("", body));
    }

    let closing_fence = format!("\n{FENCE}");
    let closing_pos = after_opening.find(&closing_fence)?;
    let yaml_text = &after_opening[..closing_pos];
    let after_closing = &after_opening[closing_pos + closing_fence.len()..];
    let body = after_closing.strip_prefix('\n').unwrap_or(after_closing);

    Some((yaml_text, body))
}

/// Extract a single frontmatter value by key.
///
/// Given this file:
///
/// ```md
/// ---
/// name: TestAgent
/// version: 0.1.0
/// model: fast
/// ---
///
/// Body content.
/// ```
///
/// `frontmatter_value(content, "name")` returns:
///   - `Some("TestAgent")`
///
/// `frontmatter_value(content, "missing")` returns:
///   - `None`
///
/// Supports dotted paths for nested YAML — see [`yaml::yaml_value`].
pub fn frontmatter_value(content: &str, key: &str) -> Option<String> {
    let (yaml_text, _) = split_frontmatter(content)?;
    yaml::yaml_value(yaml_text, key)
}

/// Return everything after the frontmatter fences.
///
/// Given this file:
///
/// ```md
/// ---
/// name: TestAgent
/// ---
///
/// # TestAgent
///
/// Body content.
/// ```
///
/// `frontmatter_body(content)` returns:
///   - `"\n# TestAgent\n\nBody content."`
///
/// When no frontmatter is present, the full content is returned unchanged.
pub fn frontmatter_body(content: &str) -> &str {
    match split_frontmatter(content) {
        Some((_, body)) => body,
        None => content,
    }
}

/// Extract a list value from frontmatter by key.
///
/// Given this file with a YAML sequence:
///
/// ```md
/// ---
/// claude.tools:
///   - Read
///   - Write
///   - Bash
/// ---
/// ```
///
/// `frontmatter_list(content, "claude.tools")` returns:
///   - `Some("Read, Write, Bash")`
///
/// Given this file with an inline string:
///
/// ```md
/// ---
/// tools: Read, Grep
/// ---
/// ```
///
/// `frontmatter_list(content, "tools")` returns:
///   - `Some("Read, Grep")`
///
/// See [`yaml::yaml_list`] for full behavior.
pub fn frontmatter_list(content: &str, key: &str) -> Option<String> {
    let (yaml_text, _) = split_frontmatter(content)?;
    yaml::yaml_list(yaml_text, key)
}

#[cfg(test)]
mod tests;
