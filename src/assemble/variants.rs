use crate::parse;
use std::path::{Path, PathBuf};

/// How a variant merges with its base content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Append,
    Prepend,
    Replace,
}

impl Mode {
    pub fn parse(value: &str) -> Self {
        match value {
            "append" => Self::Append,
            "prepend" => Self::Prepend,
            _ => Self::Replace,
        }
    }
}

/// Find the best variant file in qualifier directories.
///
/// Checks directories in precedence order:
///   `user/` > `provider/model/` > `provider/` > (none)
///
/// The `qualifiers` slice encodes the search path. Typical values:
///   `["user", "anthropic", "sonnet"]` checks `user/`, `anthropic/sonnet/`, `anthropic/`.
///
/// Returns the path to the first matching file, or `None`.
pub fn resolve(source_directory: &Path, filename: &str, qualifiers: &[String]) -> Option<PathBuf> {
    if qualifiers.contains(&"user".to_string()) {
        let user_path = source_directory.join("user").join(filename);
        if user_path.is_file() {
            return Some(user_path);
        }
    }

    let non_user: Vec<&String> = qualifiers.iter().filter(|q| q.as_str() != "user").collect();

    if non_user.len() >= 2 {
        let provider = non_user[0];
        let model = non_user[1];
        let model_path = source_directory.join(provider).join(model).join(filename);
        if model_path.is_file() {
            return Some(model_path);
        }
    }

    if !non_user.is_empty() {
        let provider = non_user[0];
        let provider_path = source_directory.join(provider).join(filename);
        if provider_path.is_file() {
            return Some(provider_path);
        }
    }

    None
}

/// Merge base content with a variant using the specified mode.
///
/// - `Append` — variant body follows base body
/// - `Prepend` — variant body precedes base body
/// - `Replace` — variant body replaces base body entirely
pub fn apply(base_content: &str, variant_content: &str, mode: Mode) -> String {
    let base_body = parse::frontmatter_body(base_content);
    let variant_body = parse::frontmatter_body(variant_content);

    match mode {
        Mode::Append => format!("{base_body}\n{variant_body}"),
        Mode::Prepend => format!("{variant_body}\n{base_body}"),
        Mode::Replace => variant_body.to_string(),
    }
}
