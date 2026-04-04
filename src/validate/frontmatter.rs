use regex::Regex;

use crate::parse;
use crate::yaml;

use super::{Diagnostic, Severity};

/// Validate frontmatter fields against a YAML schema definition.
///
/// The schema uses a simplified JSON Schema subset:
///
/// ```yaml
/// required: [name, description]
/// properties:
///     name:
///         type: string
///         pattern: "^[A-Z][a-zA-Z0-9]{2,50}$"
///     description:
///         type: string
///     version:
///         type: string
/// ```
///
/// Checks performed:
/// - Each field listed in `required` must exist in the frontmatter
/// - Fields with a `pattern` constraint must match the regex
/// - Missing optional fields are silently ignored
///
/// # Examples
///
/// Valid agent frontmatter:
///
/// ```
/// use commands::validate::validate_frontmatter;
///
/// let content = "---\nname: SecurityArchitect\ndescription: reviews code\n---\n# Body\n";
/// let schema = "required: [name, description]\nproperties:\n    name:\n        type: string\n";
/// let diagnostics = validate_frontmatter(content, schema, "agents/SecurityArchitect.md");
/// assert!(diagnostics.is_empty());
/// ```
///
/// Missing required field:
///
/// ```
/// use commands::validate::validate_frontmatter;
///
/// let content = "---\nversion: 0.1.0\n---\n# Body\n";
/// let schema = "required: [name, description]";
/// let diagnostics = validate_frontmatter(content, schema, "agents/broken.md");
/// assert_eq!(diagnostics.len(), 2);  // name and description both missing
/// ```
pub fn validate_frontmatter(
    content: &str,
    schema_content: &str,
    file_path: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let Some((yaml_text, _)) = parse::split_frontmatter(content) else {
        if has_required_fields(schema_content) {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Error,
                message: "missing frontmatter".to_string(),
            });
        }
        return diagnostics;
    };

    check_required_fields(yaml_text, schema_content, file_path, &mut diagnostics);
    check_pattern_constraints(yaml_text, schema_content, file_path, &mut diagnostics);

    diagnostics
}

fn has_required_fields(schema_content: &str) -> bool {
    yaml::yaml_list(schema_content, "required").is_some()
}

fn check_required_fields(
    yaml_text: &str,
    schema_content: &str,
    file_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(required_csv) = yaml::yaml_list(schema_content, "required") else {
        return;
    };

    for field in required_csv.split(", ") {
        let field = field.trim();
        if field.is_empty() {
            continue;
        }

        if yaml::yaml_value(yaml_text, field).is_none() {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Error,
                message: format!("missing required field '{field}'"),
            });
        }
    }
}

fn check_pattern_constraints(
    yaml_text: &str,
    schema_content: &str,
    file_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Ok(parsed): Result<serde_yaml::Value, _> = serde_yaml::from_str(schema_content) else {
        return;
    };

    let Some(properties) = parsed
        .get("properties")
        .and_then(serde_yaml::Value::as_mapping)
    else {
        return;
    };

    for (key, prop_def) in properties {
        let Some(field_name) = key.as_str() else {
            continue;
        };

        let Some(pattern) = prop_def.get("pattern").and_then(serde_yaml::Value::as_str) else {
            continue;
        };

        let Some(field_value) = yaml::yaml_value(yaml_text, field_name) else {
            continue;
        };

        let Ok(re) = Regex::new(pattern) else {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Warning,
                message: format!("invalid pattern '{pattern}' for field '{field_name}'"),
            });
            continue;
        };

        if !re.is_match(&field_value) {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Error,
                message: format!(
                    "field '{field_name}' value '{field_value}' does not match pattern '{pattern}'"
                ),
            });
        }
    }
}
