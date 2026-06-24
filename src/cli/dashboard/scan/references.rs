//! Broken-reference detection and artifact staleness/age helpers.

use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub(super) fn truncate_summary(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max).collect();
    match truncated.rfind(' ') {
        Some(index) => format!("{}…", &truncated[..index]),
        None => format!("{truncated}…"),
    }
}

/// Days since the most recent commit touching the artifact, parsed from a git
/// `%ai` date string (e.g. `2026-04-10 08:18:01 +0000`). `None` when there is no
/// history or the date does not parse.
pub(super) fn commit_age_days(date_str: &str) -> Option<i64> {
    let parsed = chrono::DateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S %z").ok()?;
    Some((chrono::Utc::now() - parsed.with_timezone(&chrono::Utc)).num_days())
}

/// Unique intra-repo markdown link targets cited by `raw_source` that no longer
/// resolve on disk. Mirrors the validated resolver: code spans/fences stripped,
/// inline `](target)` + reference-style `[label]: target` extracted, only
/// relative path-shaped targets kept (URL-decoded), resolved against the
/// artifact's own directory and the repo root. External links are not checked.
pub(super) fn broken_references(
    repo_root: &Path,
    artifact_dir: &Path,
    raw_source: &str,
) -> Vec<String> {
    let stripped = strip_code(raw_source);
    let mut broken = Vec::new();
    let mut seen = HashSet::new();
    for raw_target in link_targets(&stripped) {
        let Some(target) = normalize_reference(&raw_target) else {
            continue;
        };
        if !seen.insert(target.clone()) {
            continue;
        }
        if !reference_resolves(repo_root, artifact_dir, &target) {
            broken.push(target);
        }
    }
    broken
}

/// Removes fenced code blocks (line-delimited ```` ``` ````) and inline code
/// spans so example link syntax inside documentation is not mistaken for a real
/// reference.
fn strip_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        out.push_str(&strip_inline_code(line));
        out.push('\n');
    }
    out
}

/// Drops backtick-delimited inline code, keeping the prose between spans.
fn strip_inline_code(line: &str) -> String {
    line.split('`').step_by(2).collect::<String>()
}

/// Inline `](target)` and reference-style `[label]: target` link targets.
fn link_targets(text: &str) -> Vec<String> {
    static INLINE: OnceLock<Regex> = OnceLock::new();
    static REFDEF: OnceLock<Regex> = OnceLock::new();
    let inline = INLINE.get_or_init(|| Regex::new(r"\]\(([^)\s]+)\)").expect("valid regex"));
    let refdef =
        REFDEF.get_or_init(|| Regex::new(r"(?m)^\[[^\]]+\]:\s*(\S+)").expect("valid regex"));
    inline
        .captures_iter(text)
        .chain(refdef.captures_iter(text))
        .map(|capture| capture[1].to_string())
        .collect()
}

/// Link-target prefixes that are not local file references.
const EXTERNAL_REFERENCE_PREFIXES: [&str; 6] =
    ["http://", "https://", "mailto:", "tel:", "<", "//"];

/// Keeps only relative, path-shaped link targets (a slash or a file extension),
/// dropping anchors, external schemes, and prose. Returns the URL-decoded path.
fn normalize_reference(raw: &str) -> Option<String> {
    let target = raw.split('#').next().unwrap_or("");
    if target.is_empty() {
        return None;
    }
    if EXTERNAL_REFERENCE_PREFIXES
        .iter()
        .any(|prefix| target.starts_with(prefix))
    {
        return None;
    }
    let decoded = percent_decode(target);
    let has_extension = Path::new(&decoded)
        .extension()
        .is_some_and(|extension| !extension.is_empty());
    if !decoded.contains('/') && !has_extension {
        return None;
    }
    Some(decoded)
}

/// True when `target` resolves either relative to the artifact's directory or
/// relative to the repo root. `Path::exists` follows `..` and symlinks.
fn reference_resolves(repo_root: &Path, artifact_dir: &Path, target: &str) -> bool {
    artifact_dir.join(target).exists() || repo_root.join(target.trim_start_matches('/')).exists()
}

/// Decodes `%XX` percent-escapes (e.g. `%20` -> space) so encoded paths match
/// real filenames; leaves malformed escapes untouched.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            out.push((high << 4) | low);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Computes reference integrity + age for one artifact, given its repo (if the
/// source is locally available) and source-relative path.
pub(super) fn artifact_staleness(
    repo: Option<&PathBuf>,
    relative_path: &str,
    raw_source: &str,
    latest_commit_date: &str,
) -> (Vec<String>, Option<i64>) {
    let broken = repo.map_or_else(Vec::new, |repo_root| {
        let parent = Path::new(relative_path).parent();
        let artifact_dir =
            parent.map_or_else(|| repo_root.clone(), |relative| repo_root.join(relative));
        broken_references(repo_root, &artifact_dir, raw_source)
    });
    (broken, commit_age_days(latest_commit_date))
}
