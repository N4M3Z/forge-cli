use std::fs;
use std::path::Path;

use super::templates;

const AGENT_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/agent.schema.yaml"
));
const SKILL_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/skill.schema.yaml"
));
const RULE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/rule.schema.yaml"
));
const MODULE_SCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/module.schema.yaml"
));

pub fn embedded_schema(kind: &str) -> Option<&'static str> {
    match kind {
        "agents" => Some(AGENT_SCHEMA),
        "skills" => Some(SKILL_SCHEMA),
        "rules" => Some(RULE_SCHEMA),
        "module" => Some(MODULE_SCHEMA),
        _ => None,
    }
}

/// Load `.schema.yaml` from a directory if present.
///
/// Provider-specific schema files define required frontmatter fields
/// and pattern constraints. For example, `agents/.schema.yaml` might
/// require `name` matching `PascalCase`:
///
/// ```yaml
/// required: [name, description]
/// properties:
///     name:
///         type: string
///         pattern: "^[A-Z][a-zA-Z0-9]{2,50}$"
/// ```
///
/// Returns `None` when no `.schema.yaml` exists in the directory.
pub fn load_schema(dir: &Path) -> Option<String> {
    let schema_path = dir.join(".schema.yaml");
    fs::read_to_string(&schema_path).ok()
}

/// Load `.mdschema` from a directory if present.
///
/// The `.mdschema` file defines structural constraints for markdown
/// files in the directory — required frontmatter fields, heading rules,
/// and section structure:
///
/// ```yaml
/// frontmatter:
///     fields:
///         - name: status
///           type: string
/// heading_rules:
///     no_skip_levels: true
///     max_depth: 3
/// ```
///
/// Returns `None` when no `.mdschema` exists in the directory.
pub fn load_mdschema(dir: &Path) -> Option<String> {
    let mdschema_path = dir.join(".mdschema");
    fs::read_to_string(&mdschema_path).ok()
}

/// Load `.mdschema` from a directory, falling back to the embedded template.
///
/// Checks for a local `.mdschema` first. If missing, returns the
/// embedded template for the content kind without writing to disk.
pub fn load_mdschema_or_fallback(directory: &Path, kind: &str) -> Option<String> {
    if let Some(content) = load_mdschema(directory) {
        return Some(content);
    }
    templates::embedded_mdschema(kind)
}
