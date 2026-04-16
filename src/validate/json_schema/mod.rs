use crate::parse;

use super::{Diagnostic, Severity};

pub fn validate_frontmatter_against_json_schema(
    content: &str,
    schema_content: &str,
    file_path: &str,
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    let Some((yaml_text, _)) = parse::split_frontmatter(content) else {
        return diagnostics;
    };

    let frontmatter: serde_yaml::Value = match serde_yaml::from_str(yaml_text) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Error,
                message: format!("invalid YAML frontmatter: {error}"),
            });
            return diagnostics;
        }
    };

    let json_value: serde_json::Value = match serde_json::to_value(&frontmatter) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Error,
                message: format!("cannot convert frontmatter to JSON: {error}"),
            });
            return diagnostics;
        }
    };

    let schema: serde_json::Value = match serde_json::from_str(schema_content) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Warning,
                message: format!("invalid JSON schema: {error}"),
            });
            return diagnostics;
        }
    };

    let validator = match jsonschema::options()
        .should_validate_formats(true)
        .build(&schema)
    {
        Ok(validator) => validator,
        Err(error) => {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Warning,
                message: format!("cannot compile JSON schema: {error}"),
            });
            return diagnostics;
        }
    };

    for error in validator.iter_errors(&json_value) {
        diagnostics.push(Diagnostic {
            file: file_path.to_string(),
            line: None,
            severity: Severity::Error,
            message: format!("{}: {}", error.instance_path(), error),
        });
    }

    diagnostics
}

#[cfg(test)]
mod tests;
