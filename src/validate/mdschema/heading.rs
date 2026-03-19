use std::sync::OnceLock;

use regex::Regex;

use crate::parse;

use super::{Diagnostic, Severity};

pub(crate) struct Heading {
    pub(crate) body_line: usize,
    pub(crate) level: usize,
    pub(crate) text: String,
}

fn heading_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^(#{1,6})\s+(.+)$").expect("valid regex"))
}

pub(crate) fn extract_headings(body: &str) -> Vec<Heading> {
    let re = heading_regex();
    let mut headings = Vec::new();

    for (line_idx, line) in body.lines().enumerate() {
        if let Some(caps) = re.captures(line) {
            headings.push(Heading {
                body_line: line_idx + 1,
                level: caps[1].len(),
                text: caps[2].trim().to_string(),
            });
        }
    }

    headings
}

fn body_line_offset(file_content: &str) -> usize {
    match parse::split_frontmatter(file_content) {
        Some((yaml_text, _)) => {
            let prefix_len = 4 + yaml_text.len() + 4;
            let bounded = prefix_len.min(file_content.len());
            file_content[..bounded].lines().count()
        }
        None => 0,
    }
}

pub(super) fn check(
    file_content: &str,
    file_path: &str,
    schema: &serde_yaml::Value,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(heading_rules) = schema.get("heading_rules") else {
        return;
    };

    let no_skip_levels = heading_rules
        .get("no_skip_levels")
        .and_then(serde_yaml::Value::as_bool)
        .unwrap_or(false);

    let max_depth = heading_rules
        .get("max_depth")
        .and_then(serde_yaml::Value::as_u64)
        .map(|d| usize::try_from(d).unwrap_or(usize::MAX));

    let body = parse::frontmatter_body(file_content);
    let headings = extract_headings(body);
    let offset = body_line_offset(file_content);

    let mut prev_level: Option<usize> = None;

    for heading in &headings {
        let file_line = heading.body_line + offset;

        if let Some(max) = max_depth
            && heading.level > max
        {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: Some(file_line),
                severity: Severity::Error,
                message: format!(
                    "heading '{}' at depth {} exceeds max_depth {}",
                    heading.text, heading.level, max
                ),
            });
        }

        if no_skip_levels
            && let Some(prev) = prev_level
            && heading.level > prev + 1
        {
            diagnostics.push(Diagnostic {
                file: file_path.to_string(),
                line: Some(file_line),
                severity: Severity::Error,
                message: format!(
                    "heading '{}' skips from h{} to h{}",
                    heading.text, prev, heading.level
                ),
            });
        }

        prev_level = Some(heading.level);
    }
}
