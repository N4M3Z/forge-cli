use regex::Regex;

/// Validate that a name matches a given regex pattern.
///
/// Each provider defines its own naming convention via a `pattern` field
/// in the schema. Claude Code agents use `PascalCase` (`^[A-Z][a-zA-Z0-9]{2,50}$`),
/// but other providers may accept kebab-case, `snake_case`, or freeform names.
///
/// # Examples
///
/// `PascalCase` (Claude Code agents):
///
/// ```
/// use commands::validate::validate;
///
/// let pascal = r"^[A-Z][a-zA-Z0-9]{2,50}$";
/// assert!(validate("SecurityArchitect", pascal).is_ok());
/// assert!(validate("QA", pascal).is_err());          // too short
/// assert!(validate("my-agent", pascal).is_err());    // not PascalCase
/// ```
///
/// Kebab-case (hypothetical provider):
///
/// ```
/// use commands::validate::validate;
///
/// let kebab = r"^[a-z][a-z0-9-]{1,49}$";
/// assert!(validate("code-reviewer", kebab).is_ok());
/// assert!(validate("CodeReviewer", kebab).is_err());
/// ```
pub fn validate(name: &str, pattern: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("name must not be empty".to_string());
    }

    let re =
        Regex::new(pattern).map_err(|err| format!("invalid name pattern '{pattern}': {err}"))?;

    if !re.is_match(name) {
        return Err(format!(
            "name '{name}' does not match required pattern '{pattern}'"
        ));
    }

    Ok(())
}
