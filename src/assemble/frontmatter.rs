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
    let mut keep_current_block = false;
    for line in yaml_text.lines() {
        if let Some(key) = top_level_key(line) {
            keep_current_block = keep_fields
                .iter()
                .any(|field| key.eq_ignore_ascii_case(field));
            if keep_current_block {
                kept_lines.push(line);
            }
        } else if keep_current_block {
            // Continuation of a kept key: a block scalar, list item, or
            // nested mapping value. Retaining it keeps multi-line values
            // (e.g. an `allowed-tools:` list) intact instead of collapsing
            // them to a bare key with no value.
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

/// Replace a field value in YAML frontmatter via `serde_yaml` round-trip.
///
/// Handles quoted values, block scalars, and inline comments correctly.
/// Key order is preserved (`serde_yaml::Mapping` uses `IndexMap`).
pub fn map_field(content: &str, target_field: &str, mapper: impl Fn(&str) -> String) -> String {
    let Some((yaml_text, body)) = crate::parse::split_frontmatter(content) else {
        return content.to_string();
    };

    let Ok(mut parsed): Result<serde_yaml::Value, _> = serde_yaml::from_str(yaml_text) else {
        return content.to_string();
    };

    let Some(mapping) = parsed.as_mapping_mut() else {
        return content.to_string();
    };

    let key = serde_yaml::Value::String(target_field.to_string());
    let Some(field_value) = mapping.get_mut(&key) else {
        return content.to_string();
    };

    if let Some(current) = field_value.as_str() {
        *field_value = serde_yaml::Value::String(mapper(current));
    }

    let Ok(new_yaml) = serde_yaml::to_string(&parsed) else {
        return content.to_string();
    };

    format!("---\n{new_yaml}---\n{body}")
}

/// Set or replace a string field in YAML frontmatter.
pub fn set_field(content: &str, target_field: &str, value: &str) -> String {
    let Some((yaml_text, body)) = crate::parse::split_frontmatter(content) else {
        return content.to_string();
    };

    let Ok(mut parsed): Result<serde_yaml::Value, _> = serde_yaml::from_str(yaml_text) else {
        return content.to_string();
    };

    let Some(mapping) = parsed.as_mapping_mut() else {
        return content.to_string();
    };

    mapping.insert(
        serde_yaml::Value::String(target_field.to_string()),
        serde_yaml::Value::String(value.to_string()),
    );

    let Ok(new_yaml) = serde_yaml::to_string(&parsed) else {
        return content.to_string();
    };

    format!("---\n{new_yaml}---\n{body}")
}

/// Return the key of a top-level YAML mapping line — one with no leading
/// indentation and a valid `key:` prefix. Indented continuation lines, list
/// items (`- ...`), comments, and blanks return `None`, marking them as part
/// of the preceding key's value rather than a new key.
fn top_level_key(line: &str) -> Option<&str> {
    if line.starts_with([' ', '\t']) {
        return None;
    }
    let colon_pos = line.find(':')?;
    let key = &line[..colon_pos];
    if key.is_empty() {
        return None;
    }
    let is_valid_key = key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.');
    is_valid_key.then_some(key)
}

/// Strip a leading `# Title` heading if it's the first non-empty line.
fn strip_heading(text: &str) -> String {
    let had_newline = text.ends_with('\n');
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
            let mut result = trimmed.to_string();
            super::restore_trailing_newline(&mut result, had_newline);
            return result;
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
        super::restore_trailing_newline(&mut output, had_newline);
        return output;
    }

    let mut result = skipped_blanks.join("\n");
    super::restore_trailing_newline(&mut result, had_newline);
    result
}
