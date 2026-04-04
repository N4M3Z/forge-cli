mod check;
mod schema;

use commands::error::{Error, ErrorKind};
use commands::result::ActionResult;
use std::fs;
use std::path::Path;

use crate::cli::config;

/// Validate module structure and content files against schemas.
///
/// Checks:
///   - Required/optional files from validation config
///   - agents/, rules/ — frontmatter against `.schema.yaml`, structure against `.mdschema`
///   - skills/ — recurses into subdirectories, checks `.mdschema`
pub fn execute(path: &str) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let mut result = ActionResult::new();

    check_module_structure(module_root, &mut result);

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

/// Check module structure against validation config from defaults.yaml.
fn check_module_structure(module_root: &Path, result: &mut ActionResult) {
    let validation_config = config::load_validation_config(module_root);

    for filename in &validation_config.required {
        if module_root.join(filename).is_file() {
            println!("  ok {filename}");
        } else {
            result
                .errors
                .push(format!("missing required file: {filename}"));
            println!("  MISSING {filename}");
        }
    }

    for filename in &validation_config.optional {
        if module_root.join(filename).is_file() {
            println!("  ok {filename}");
        } else {
            println!("  MISSING {filename} (optional)");
        }
    }
}
