/// Convert markdown with frontmatter to TOML configuration format.
///
use serde::Serialize;

/// Extracts agent metadata from frontmatter. The body (everything after
/// frontmatter) becomes the `developer_instructions` TOML string field.
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
/// developer_instructions = """
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
    let name = crate::parse::frontmatter_value(content, "name").unwrap_or_else(|| {
        source_name
            .strip_suffix(".md")
            .unwrap_or(source_name)
            .to_string()
    });
    let description = crate::parse::frontmatter_value(content, "description").unwrap_or_default();
    let model = crate::parse::frontmatter_value(content, "model");
    let model_reasoning_effort = crate::parse::frontmatter_value(content, "effort");
    let body = crate::parse::frontmatter_body(content);

    toml::to_string(&CodexAgentToml {
        name,
        description,
        model,
        model_reasoning_effort,
        developer_instructions: body.trim().to_string(),
    })
    .map(|toml| format!("# source: {source_name}\n{toml}"))
    .map_err(|error| format!("failed to serialize TOML for {source_name}: {error}"))
}

#[derive(Serialize)]
struct CodexAgentToml {
    name: String,
    description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model_reasoning_effort: Option<String>,
    developer_instructions: String,
}
