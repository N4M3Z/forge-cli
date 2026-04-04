use serde::Serialize;

mod agent;
mod frontmatter;
pub mod mdschema;

pub use agent::validate;
pub use frontmatter::validate_frontmatter;

// --- Types ---

/// A single validation finding with location, severity, and message.
///
/// ```
/// use commands::validate::{Diagnostic, Severity};
///
/// let diag = Diagnostic {
///     file: "agents/MyAgent.md".to_string(),
///     line: Some(5),
///     severity: Severity::Error,
///     message: "missing required field 'name'".to_string(),
/// };
///
/// assert_eq!(diag.severity, Severity::Error);
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostic {
    pub file: String,
    pub line: Option<usize>,
    pub severity: Severity,
    pub message: String,
}

/// Severity level for a validation diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Error,
    Warning,
}

/// A content file paired with its path, ready for validation.
///
/// Bundles a file's content and path so callers don't need to pass both
/// separately to each validation function. The path is used only for
/// diagnostic output — it is not read from disk.
///
/// ```
/// use commands::validate::ContentFile;
///
/// let file = ContentFile {
///     path: "agents/MyAgent.md",
///     content: "---\nname: MyAgent\n---\n# MyAgent\n",
/// };
/// ```
pub struct ContentFile<'a> {
    pub path: &'a str,
    pub content: &'a str,
}

#[cfg(test)]
mod tests;
