mod check;
mod schema;
mod templates;
mod tools;

use commands::error::{Error, ErrorKind};
use commands::result::ActionResult;
use std::fs;
use std::path::Path;

const REQUIRED_FILES: &[&str] = &["module.yaml", "defaults.yaml", "README.md", "LICENSE"];
const OPTIONAL_FILES: &[&str] = &[
    "INSTALL.md",
    "CONTRIBUTING.md",
    "CODEOWNERS",
    "CHANGELOG.md",
    ".gitattributes",
];

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
    check_module_yaml(module_root, &mut result);

    for kind in &["agents", "rules"] {
        let dir = module_root.join(kind);
        if dir.is_dir() {
            check::flat_directory(&dir, module_root, kind, &mut result)?;
        }
    }

    // ADR directory
    let decisions_dir = module_root.join("docs").join("decisions");
    if decisions_dir.is_dir() {
        check::flat_directory(&decisions_dir, module_root, "decisions", &mut result)?;
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

    tools::run_external_checks(module_root, &mut result);

    Ok(result)
}

fn check_module_structure(module_root: &Path, result: &mut ActionResult) {
    for filename in REQUIRED_FILES {
        if module_root.join(filename).is_file() {
            println!("  ok {filename}");
        } else {
            result
                .errors
                .push(format!("missing required file: {filename}"));
            println!("  MISSING {filename}");
        }
    }

    for filename in OPTIONAL_FILES {
        if module_root.join(filename).is_file() {
            println!("  ok {filename}");
        } else {
            println!("  MISSING {filename} (optional)");
        }
    }
}

fn check_module_yaml(module_root: &Path, result: &mut ActionResult) {
    let module_yaml_path = module_root.join("module.yaml");
    if !module_yaml_path.is_file() {
        return;
    }

    let Some(module_schema) = schema::embedded_schema("module") else {
        return;
    };

    let Ok(content) = fs::read_to_string(&module_yaml_path) else {
        return;
    };

    let yaml_as_frontmatter = format!("---\n{content}---\n");
    let diagnostics = commands::validate::validate_frontmatter(
        &yaml_as_frontmatter,
        module_schema,
        "module.yaml",
    );

    for diagnostic in diagnostics {
        result.errors.push(format!(
            "{}: {} ({:?})",
            diagnostic.file, diagnostic.message, diagnostic.severity
        ));
    }
}

#[cfg(test)]
mod tests;
