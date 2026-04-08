use crate::parse;

/// Reduce frontmatter to only the fields listed in `keep_fields`.
///
/// When `keep_fields` is empty, all frontmatter is stripped.
/// When fields are specified, only matching top-level keys survive
/// and the fences are preserved around them.
///
/// The leading `# Title` heading is also stripped if present
/// immediately after the frontmatter.
pub fn strip_frontmatter(content: &str, keep_fields: &[&str]) -> String {
    let Some((yaml_text, body)) = parse::split_frontmatter(content) else {
        return strip_heading(content);
    };

    if keep_fields.is_empty() {
        return strip_heading(body);
    }

    let mut kept_lines: Vec<&str> = Vec::new();
    for line in yaml_text.lines() {
        let Some(colon_pos) = line.find(':') else {
            continue;
        };
        let key = &line[..colon_pos];
        let is_valid_key = key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
        if !is_valid_key {
            continue;
        }
        let mut matched = false;
        for field in keep_fields {
            if key.eq_ignore_ascii_case(field) {
                matched = true;
                break;
            }
        }
        if matched {
            kept_lines.push(line);
        }
    }

    let stripped_body = strip_heading(body);

    if kept_lines.is_empty() {
        return stripped_body;
    }

    let mut output = String::new();
    output.push_str("---\n");
    for line in &kept_lines {
        output.push_str(line);
        output.push('\n');
    }
    output.push_str("---\n");
    output.push_str(&stripped_body);
    output
}

/// Surgically replace a field value in YAML frontmatter.
///
/// Preserves comments, order, and whitespace. Key matching is case-insensitive.
pub fn map_field(content: &str, target_field: &str, mapper: impl Fn(&str) -> String) -> String {
    let Some((yaml_text, body)) = crate::parse::split_frontmatter(content) else {
        return content.to_string();
    };

    let mut new_yaml = String::with_capacity(yaml_text.len());
    let mut found = false;

    for line in yaml_text.lines() {
        if !found && let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            if key.eq_ignore_ascii_case(target_field) {
                let after_colon = &line[colon_pos + 1..];
                let value = after_colon.trim();

                // Find existing spacing after colon to preserve it
                let spacing = &after_colon[..after_colon.len() - after_colon.trim_start().len()];
                let spacing = if spacing.is_empty() { " " } else { spacing };

                new_yaml.push_str(&line[..=colon_pos]);
                new_yaml.push_str(spacing);
                new_yaml.push_str(&mapper(value));
                new_yaml.push('\n');
                found = true;
                continue;
            }
        }
        new_yaml.push_str(line);
        new_yaml.push('\n');
    }

    if !found {
        return content.to_string();
    }

    let mut output = String::with_capacity(content.len() + 8);
    output.push_str("---\n");
    output.push_str(&new_yaml);
    output.push_str("---\n");
    output.push_str(body);
    output
}

/// Strip a leading `# Title` heading if it's the first non-empty line.
fn strip_heading(text: &str) -> String {
    let mut lines = text.lines();
    let mut skipped_blanks: Vec<&str> = Vec::new();

    for line in &mut lines {
        if line.is_empty() {
            skipped_blanks.push(line);
            continue;
        }
        if line.starts_with("# ") {
            let rest: String = lines.collect::<Vec<&str>>().join("\n");
            let trimmed = rest.strip_prefix('\n').unwrap_or(&rest);
            return trimmed.to_string();
        }
        // First non-empty line is not a heading — return everything.
        let mut output = String::new();
        for blank in &skipped_blanks {
            output.push_str(blank);
            output.push('\n');
        }
        output.push_str(line);
        for remaining in lines {
            output.push('\n');
            output.push_str(remaining);
        }
        return output;
    }

    skipped_blanks.join("\n")
}
