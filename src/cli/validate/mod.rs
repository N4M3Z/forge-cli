mod check;
mod schema;

use commands::error::{Error, ErrorKind};
use commands::result::ActionResult;
use std::fs;
use std::path::Path;

/// Validate module files against their schemas.
///
/// Walks source directories and checks:
///   - agents/, rules/ — frontmatter validation against `.schema.yaml`,
///     structure validation against `.mdschema`
///   - skills/ — recurses into subdirectories, checks `.mdschema`
///
/// ```text
/// module/
///   agents/
///     .schema.yaml               ← frontmatter constraints
///     SecurityArchitect.md       ← validated
///   rules/
///     .mdschema                  ← heading/structure constraints
///     MyRule.md                  ← validated
///   skills/
///     Explain/
///       .mdschema                ← per-skill constraints
///       SKILL.md                 ← validated
/// ```
///
/// All findings are collected into `ActionResult.errors`.
pub fn execute(path: &str) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let mut result = ActionResult::new();

    for kind in &["agents", "rules"] {
        let dir = module_root.join(kind);
        if dir.is_dir() {
            check::flat_directory(&dir, module_root, &mut result)?;
        }
    }

    // Skills have subdirectories — iterate and validate each
    let skills_dir = module_root.join("skills");
    if skills_dir.is_dir() {
        let entries = fs::read_dir(&skills_dir).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {e}", skills_dir.display()),
            )
        })?;

        for entry in entries {
            let entry = entry
                .map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;

            let path = entry.path();
            if path.is_dir() {
                check::skill_directory(&path, &mut result)?;
            }
        }
    }

    Ok(result)
}
