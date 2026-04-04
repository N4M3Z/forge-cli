use commands::error::{Error, ErrorKind};
use commands::result::{ActionResult, DeployedFile};
use std::fs;
use std::path::Path;

/// Copy source files directly to a target directory.
///
/// No assembly, no transforms, no manifest tracking.
/// Copies agents/, skills/, rules/ as-is from module root to target.
pub fn execute(path: &str, target: &str) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let target_root = Path::new(target);
    let mut result = ActionResult::new();

    for kind in commands::provider::ContentKind::ALL {
        let kind_string = kind.as_str();
        let source_directory = module_root.join(kind_string);
        if !source_directory.is_dir() {
            continue;
        }

        let target_directory = target_root.join(kind_string);

        copy_directory_recursive(
            &source_directory,
            &target_directory,
            kind_string,
            &mut result,
        )?;
    }

    Ok(result)
}

fn copy_directory_recursive(
    source_directory: &Path,
    target_directory: &Path,
    kind: &str,
    result: &mut ActionResult,
) -> Result<(), Error> {
    let entries = fs::read_dir(source_directory).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {}: {error}", source_directory.display()),
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            Error::new(ErrorKind::Io, format!("directory entry error: {error}"))
        })?;

        let source_path = entry.path();
        let filename = source_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let target_path = target_directory.join(&filename);

        if source_path.is_dir() {
            copy_directory_recursive(&source_path, &target_path, kind, result)?;
            continue;
        }

        if source_path.extension().unwrap_or_default() != "md" {
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("cannot create {}: {error}", parent.display()),
                )
            })?;
        }

        fs::copy(&source_path, &target_path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(
                    "cannot copy {} -> {}: {error}",
                    source_path.display(),
                    target_path.display()
                ),
            )
        })?;

        result.installed.push(DeployedFile {
            source: source_path.to_string_lossy().to_string(),
            target: target_path.to_string_lossy().to_string(),
            provider: kind.to_string(),
        });
    }

    Ok(())
}
