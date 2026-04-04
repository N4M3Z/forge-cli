use crate::parse;
use crate::yaml;

use super::{Diagnostic, Severity};

pub(super) fn check(
    file_content: &str,
    file_path: &str,
    schema: &serde_yaml::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(fields) = schema
        .get("frontmatter")
        .and_then(|fm| fm.get("fields"))
        .and_then(serde_yaml::Value::as_sequence)
    else {
        return;
    };

    let Some((yaml_text, _)) = parse::split_frontmatter(file_content) else {
        diagnostics.push(Diagnostic {
            file: file_path.to_string(),
            line: None,
            severity: Severity::Error,
            message: "missing frontmatter".to_string(),
        });
        return;
    };

    for field_def in fields {
        let is_optional = field_def
            .get("optional")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false);

        if is_optional {
            continue;
        }

        let Some(field_name) = field_def.get("name").and_then(serde_yaml::Value::as_str) else {
            continue;
        };

        let has_scalar = yaml::yaml_value(yaml_text, field_name).is_some();
        let has_list = yaml::yaml_list(yaml_text, field_name).is_some();

        if !has_scalar && !has_list {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Error,
                message: format!("missing required frontmatter field '{field_name}'"),
            });
        }
    }
}
