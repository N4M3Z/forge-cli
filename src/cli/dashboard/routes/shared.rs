use axum::response::{Html, IntoResponse};

/// Strips a `.md` or `.toml` extension, so the same artifact deployed as
/// `SKILL.md` (claude) and `SKILL.toml` (codex) compares equal.
pub(super) fn strip_extension(path: &str) -> &str {
    path.strip_suffix(".md")
        .or_else(|| path.strip_suffix(".toml"))
        .unwrap_or(path)
}

/// Returns the value only when it is an http(s) URL, else an empty string, so a
/// `javascript:`/`data:`/`file:` value from a sidecar `source:` or a module's
/// `repository:` never reaches an anchor href. The templates already hide the
/// link when this is empty.
pub(super) fn http_uri(value: &str) -> &str {
    if value.starts_with("https://") || value.starts_with("http://") {
        value
    } else {
        ""
    }
}

/// Normalizes a source URI for provenance correlation: trims a trailing slash
/// and `.git` so the same repository compares equal across the spelling
/// variations sidecars and git origins use. Distinct repositories that merely
/// share a basename stay distinct, unlike a basename-only key.
pub(super) fn canonical_source(uri: &str) -> &str {
    uri.trim_end_matches('/')
        .trim_end_matches(".git")
        .trim_end_matches('/')
}

/// Renders an absolute path with the home directory abbreviated to `~`.
pub(super) fn display_path(path: &std::path::Path, home: Option<&std::path::Path>) -> String {
    if let Some(home) = home
        && let Ok(rest) = path.strip_prefix(home)
    {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

pub(super) fn not_found(message: &str) -> axum::response::Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        Html(format!("<p>{message}</p>")),
    )
        .into_response()
}
