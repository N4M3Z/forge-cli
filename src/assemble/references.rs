/// Remove reference-style link definitions (`[1]: url`, `[MADR]: url`) and
/// inline reference markers (` [1]`, ` [MADR]`) from content.
pub fn strip(content: &str) -> String {
    let had_newline = content.ends_with('\n');

    static INLINE_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static DEF_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let inline_re =
        INLINE_RE.get_or_init(|| regex::Regex::new(r" \[[\w][\w-]*\]").expect("valid regex"));
    let def_re =
        DEF_RE.get_or_init(|| regex::Regex::new(r"^\[[\w][\w-]*\]:").expect("valid regex"));

    let mut output_lines: Vec<String> = Vec::new();
    let mut in_ref_block = false;

    for line in content.lines() {
        if def_re.is_match(line) {
            in_ref_block = true;
            continue;
        }
        if in_ref_block && (line.is_empty() || line.starts_with('[')) {
            continue;
        }
        in_ref_block = false;
        let cleaned = inline_re.replace_all(line, "").to_string();
        output_lines.push(cleaned);
    }

    while output_lines.last().is_some_and(String::is_empty) {
        output_lines.pop();
    }

    let mut result = output_lines.join("\n");
    super::restore_trailing_newline(&mut result, had_newline);
    result
}

/// Extract reference-style link URLs from content.
///
/// Parses lines matching `[N]: <url>` and returns the URLs
/// in the order they appear.
pub fn extract(content: &str) -> Vec<String> {
    static URL_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

    let url_re =
        URL_RE.get_or_init(|| regex::Regex::new(r"^\[[\w][\w-]*\]:\s*(\S+)").expect("valid regex"));

    let mut urls: Vec<String> = Vec::new();
    for line in content.lines() {
        if let Some(caps) = url_re.captures(line) {
            urls.push(caps[1].to_string());
        }
    }
    urls
}
