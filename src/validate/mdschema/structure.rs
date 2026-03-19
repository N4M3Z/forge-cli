use regex::Regex;

use crate::parse;

use super::heading::{Heading, extract_headings};
use super::{Diagnostic, Severity};

pub(super) fn check(
    file_content: &str,
    file_path: &str,
    schema: &serde_yaml::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(structure) = schema
        .get("structure")
        .and_then(serde_yaml::Value::as_sequence)
    else {
        return;
    };

    let body = parse::frontmatter_body(file_content);
    let headings = extract_headings(body);

    check_required_sections(structure, &headings, file_path, diagnostics);
}

fn check_required_sections(
    structure: &[serde_yaml::Value],
    headings: &[Heading],
    file_path: &str,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for section_def in structure {
        let is_optional = section_def
            .get("optional")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false);

        if is_optional {
            continue;
        }

        let Some(heading_def) = section_def.get("heading") else {
            continue;
        };

        let Some(pattern) = heading_def
            .get("pattern")
            .and_then(serde_yaml::Value::as_str)
        else {
            continue;
        };

        let is_regex = heading_def
            .get("regex")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false);

        let found = heading_matches_any(pattern, is_regex, headings);

        if !found {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: None,
                severity: Severity::Error,
                message: format!("missing required section matching '{pattern}'"),
            });
        }

        if let Some(children) = section_def
            .get("children")
            .and_then(serde_yaml::Value::as_sequence)
        {
            check_required_sections(children, headings, file_path, diagnostics);
        }
    }
}

fn heading_matches_any(pattern: &str, is_regex: bool, headings: &[Heading]) -> bool {
    if is_regex {
        let Ok(re) = Regex::new(pattern) else {
            return false;
        };

        for heading in headings {
            let full = format!("{} {}", "#".repeat(heading.level), heading.text);
            if re.is_match(&full) {
                return true;
            }
        }
    } else {
        for heading in headings {
            let full = format!("{} {}", "#".repeat(heading.level), heading.text);
            if full == pattern {
                return true;
            }
        }
    }

    false
}
