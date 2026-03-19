use std::fs;
use std::path::Path;

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
