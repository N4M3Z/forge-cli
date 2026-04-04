mod merge;

pub use merge::deep_merge;
use serde_yaml::Value;

/// Extract a scalar value from raw YAML text using a dot-separated expression.
///
/// Expressions support dotted notation for nested access.
/// Given this YAML:
///
/// ```yaml
/// user:
///   root: /home
/// ```
///
/// `yaml_value(yaml_text, "user.root")` returns:
///   - `Some("/home")`
///
/// Flat dotted keys are tried first as an exact literal match.
/// Given this YAML:
///
/// ```yaml
/// claude.name: SecurityArchitect
/// claudeXname: Wrong
/// ```
///
/// `yaml_value(yaml_text, "claude.name")` returns:
///   - `Some("SecurityArchitect")`
///
/// Scalar types are coerced to strings:
///
/// ```yaml
/// draft: true
/// priority: 42
/// empty:
/// ```
///
/// `yaml_value(yaml_text, "draft")` returns:
///   - `Some("true")`
///
/// `yaml_value(yaml_text, "priority")` returns:
///   - `Some("42")`
///
/// `yaml_value(yaml_text, "empty")` returns:
///   - `None`
///
/// This function operates on raw YAML text, not full markdown content.
/// Assembly rules can feed yq output here instead of using the built-in
/// `serde_yaml` parser — the contract is: pass valid YAML, get the value.
pub fn yaml_value(yaml_text: &str, expression: &str) -> Option<String> {
    let parsed: Value = serde_yaml::from_str(yaml_text).ok()?;
    let resolved = resolve_expression(&parsed, expression)?;

    match resolved {
        Value::String(string) => Some(string.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        Value::Null => None,
        other => {
            let serialized = serde_yaml::to_string(other).ok()?;
            Some(serialized.trim().to_string())
        }
    }
}

/// Extract a list value from raw YAML text using a dot-separated expression.
///
/// YAML sequences are returned as comma-separated strings.
/// Given this YAML:
///
/// ```yaml
/// tags:
///   - one
///   - two
///   - three
/// ```
///
/// `yaml_list(yaml_text, "tags")` returns:
///   - `Some("one, two, three")`
///
/// A plain string value is returned as-is, which handles inline lists.
/// Given this YAML:
///
/// ```yaml
/// tools: Read, Write, Bash
/// ```
///
/// `yaml_list(yaml_text, "tools")` returns:
///   - `Some("Read, Write, Bash")`
///
/// Empty sequences return `None`:
///
/// ```yaml
/// tools: []
/// ```
///
/// `yaml_list(yaml_text, "tools")` returns:
///   - `None`
pub fn yaml_list(yaml_text: &str, expression: &str) -> Option<String> {
    let parsed: Value = serde_yaml::from_str(yaml_text).ok()?;
    let resolved = resolve_expression(&parsed, expression)?;

    match resolved {
        Value::Sequence(sequence) => {
            let mut items: Vec<String> = Vec::new();

            for value in sequence {
                match value {
                    Value::String(string) => items.push(string.clone()),
                    Value::Number(number) => items.push(number.to_string()),
                    Value::Bool(flag) => items.push(flag.to_string()),
                    _ => {}
                }
            }

            if items.is_empty() {
                None
            } else {
                Some(items.join(", "))
            }
        }
        Value::String(string) => Some(string.clone()),
        _ => None,
    }
}

/// Resolve a dot-separated expression against a parsed YAML document.
///
/// Resolution order:
///
/// 1. Try the full expression as a literal flat key.
///    Given this YAML:
///
///    ```yaml
///    claude.name: Architect
///    ```
///
///    `resolve_expression(root, "claude.name")` finds the flat key directly.
///
/// 2. Fall back to walking dot-separated segments as nested mapping keys.
///    Given this YAML:
///
///    ```yaml
///    user:
///      settings:
///        theme: dark
///    ```
///
///    `resolve_expression(root, "user.settings.theme")` walks
///    user -> settings -> theme.
///
/// Returns `None` if neither strategy finds a match.
fn resolve_expression<'yaml>(root: &'yaml Value, expression: &str) -> Option<&'yaml Value> {
    let mapping = root.as_mapping()?;

    let literal_key = Value::String(expression.to_string());
    if let Some(direct_match) = mapping.get(&literal_key) {
        return Some(direct_match);
    }

    if !expression.contains('.') {
        return None;
    }

    let segments: Vec<&str> = expression.split('.').collect();
    let mut current = root;

    for segment in &segments {
        match current.get(*segment) {
            Some(next) => current = next,
            None => return None,
        }
    }

    Some(current)
}

#[cfg(test)]
mod tests;
