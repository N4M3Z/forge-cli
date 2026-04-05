use commands::error::{Error, ErrorKind};
use commands::result::ActionResult;
use commands::validate;
use std::fs;
use std::path::Path;

use super::schema;
use crate::cli::config::read_file;

/// Validate all .md files in a flat content directory (agents/ or rules/).
///
/// For each .md file found:
///   1. If a `.schema.yaml` exists in the directory, validates frontmatter
///      fields and patterns against it
///   2. If a `.mdschema` exists, checks heading structure and section
///      requirements
///
/// Diagnostics are appended to `result.errors` as formatted strings.
pub fn flat_directory(
    dir: &Path,
    module_root: &Path,
    kind: &str,
    result: &mut ActionResult,
) -> Result<(), Error> {
    let schema_content =
        schema::load_schema(dir).or_else(|| schema::embedded_schema(kind).map(String::from));
    let mdschema_content = schema::load_mdschema_or_fallback(dir, kind)
        .map_err(|error| Error::new(ErrorKind::Io, error))?;

    let entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;

        let path = entry.path();
        if path.is_dir() || path.extension().unwrap_or_default() != "md" {
            continue;
        }

        let content = read_file(&path)?;
        let relative = path
            .strip_prefix(module_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        collect_diagnostics(
            &content,
            &relative,
            schema_content.as_ref(),
            mdschema_content.as_ref(),
            result,
        );
    }

    Ok(())
}

/// Validate .md files inside a skill subdirectory.
///
/// Skill directories contain `SKILL.md` (the main skill definition) plus
/// optional companion files (examples, references). Each file in the
/// directory is checked against the local `.mdschema` if one exists.
///
/// Unlike flat directories, skill dirs don't check `.schema.yaml` for
/// frontmatter — the skill's own frontmatter format is self-describing.
///
/// ```text
/// skills/
///   Explain/
///     SKILL.md          ← checked against skills/Explain/.mdschema
///     examples.md       ← also checked
///     .mdschema         ← structural constraints for this skill
/// ```
pub fn skill_directory(dir: &Path, result: &mut ActionResult) -> Result<(), Error> {
    let mdschema_content = schema::load_mdschema_or_fallback(dir, "skills")
        .map_err(|error| Error::new(ErrorKind::Io, error))?;

    // Only validate SKILL.md against the schema — companions are reference
    // docs without skill frontmatter (name, description, version).
    let skill_file = dir.join("SKILL.md");
    if skill_file.is_file() {
        let content = read_file(&skill_file)?;
        let display_path = skill_file.to_string_lossy().to_string();

        collect_diagnostics(
            &content,
            &display_path,
            None,
            mdschema_content.as_ref(),
            result,
        );
    }

    Ok(())
}

/// Run schema and mdschema validation, appending any diagnostics to the result.
fn collect_diagnostics(
    content: &str,
    file_path: &str,
    schema_content: Option<&String>,
    mdschema_content: Option<&String>,
    result: &mut ActionResult,
) {
    if let Some(schema) = schema_content {
        let diagnostics = validate::validate_frontmatter(content, schema, file_path);
        for diag in diagnostics {
            result.errors.push(format!(
                "{}: {} ({:?})",
                diag.file, diag.message, diag.severity
            ));
        }
    }

    if let Some(mdschema) = mdschema_content {
        let diagnostics = validate::mdschema::check(content, file_path, mdschema);
        for diag in diagnostics {
            result.errors.push(format!(
                "{}: {} ({:?})",
                diag.file, diag.message, diag.severity
            ));
        }
    }
}
