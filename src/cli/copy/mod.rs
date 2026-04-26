use commands::error::{Error, ErrorKind};
use commands::manifest;
use commands::result::{ActionResult, DeployedFile};
use std::fs;
use std::path::Path;

/// Copy source files directly to a target directory.
///
/// Copies agents/, skills/, rules/ as-is from module root to target.
/// When the source module has a `module.yaml` (and `skip_provenance` is false),
/// writes SLSA provenance sidecars to `.provenance/` directories alongside
/// each copied file in the target tree.
pub fn execute(path: &str, target: &str, skip_provenance: bool) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let target_root = Path::new(target);
    let mut result = ActionResult::new();

    let source_uri = if skip_provenance {
        String::new()
    } else {
        super::config::load_source_uri(module_root)
    };

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
            module_root,
            kind_string,
            &source_uri,
            &mut result,
        )?;
    }

    Ok(result)
}

fn copy_directory_recursive(
    source_directory: &Path,
    target_directory: &Path,
    module_root: &Path,
    kind: &str,
    source_uri: &str,
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
            copy_directory_recursive(
                &source_path,
                &target_path,
                module_root,
                kind,
                source_uri,
                result,
            )?;
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

        let content = fs::read_to_string(&source_path).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read {}: {error}", source_path.display()),
            )
        })?;

        fs::write(&target_path, &content).map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot write {}: {error}", target_path.display()),
            )
        })?;

        if !source_uri.is_empty() {
            write_copy_provenance(
                &source_path,
                &target_path,
                module_root,
                &content,
                source_uri,
            )?;
        }

        result.installed.push(DeployedFile {
            source: source_path.to_string_lossy().to_string(),
            target: target_path.to_string_lossy().to_string(),
            provider: kind.to_string(),
        });
    }

    Ok(())
}

fn write_copy_provenance(
    source_path: &Path,
    target_path: &Path,
    module_root: &Path,
    content: &str,
    source_uri: &str,
) -> Result<(), Error> {
    let relative_source = source_path
        .strip_prefix(module_root)
        .unwrap_or(source_path)
        .to_string_lossy()
        .to_string();

    let content_digest = manifest::content_sha256(content);

    let statement = manifest::generate_statement(
        &relative_source,
        &content_digest,
        &[(relative_source.clone(), content_digest.clone())],
        env!("CARGO_PKG_NAME"),
        &format!("{}/copy/v1", env!("CARGO_PKG_REPOSITORY")),
        env!("CARGO_PKG_VERSION"),
        source_uri,
    );

    let provenance_directory = target_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(manifest::PROVENANCE_DIRECTORY);

    fs::create_dir_all(&provenance_directory).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", provenance_directory.display()),
        )
    })?;

    let sidecar_filename = format!(
        "{}.{}",
        target_path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy(),
        manifest::SIDECAR_EXTENSION
    );

    let sidecar_path = provenance_directory.join(sidecar_filename);

    fs::write(&sidecar_path, &statement).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {error}", sidecar_path.display()),
        )
    })?;

    Ok(())
}

#[cfg(test)]
mod tests;
