use commands::error::{Error, ErrorKind};
use commands::manifest;
use commands::result::{ActionResult, DeployedFile, SkipReason, SkippedFile};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use super::validate::templates::InitTemplates;

pub fn execute(path: &str) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let mut result = ActionResult::new();
    let mut manifest_entries: HashMap<String, manifest::ManifestEntry> = HashMap::new();

    let module_name = resolve_module_name(module_root);
    fs::create_dir_all(module_root)
        .map_err(|error| Error::new(ErrorKind::Io, format!("cannot create {path}: {error}")))?;

    for filename in InitTemplates::iter() {
        // Skip hidden files like .DS_Store that might be in the templates directory
        if filename.starts_with('.')
            && !filename.starts_with(".githooks")
            && !filename.starts_with(".github")
        {
            continue;
        }
        let Some(data) = InitTemplates::get(&filename) else {
            continue;
        };
        let template_content = std::str::from_utf8(data.data.as_ref())
            .map_err(|error| Error::new(ErrorKind::Io, format!("{filename}: {error}")))?;
        let content = template_content
            .replace("${MODULE_NAME}", &module_name)
            .replace("${VERSION}", "0.1.0");

        let target_path = module_root.join(filename.as_ref());
        let template_hash = manifest::content_sha256(&content);

        let should_manifest = if target_path.exists() {
            let actual_content = fs::read_to_string(&target_path).map_err(|error| {
                Error::new(ErrorKind::Io, format!("{}: {error}", target_path.display()))
            })?;
            let matches_template = manifest::content_sha256(&actual_content) == template_hash;
            result.skipped.push(SkippedFile {
                target: filename.to_string(),
                provider: "init".to_string(),
                reason: SkipReason::AlreadyExists,
            });
            matches_template
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    Error::new(ErrorKind::Io, format!("{}: {error}", parent.display()))
                })?;
            }
            fs::write(&target_path, &content).map_err(|error| {
                Error::new(ErrorKind::Io, format!("{}: {error}", target_path.display()))
            })?;

            result.installed.push(DeployedFile {
                source: format!("templates/init/{filename}"),
                target: filename.to_string(),
                provider: "init".to_string(),
            });
            true
        };

        if should_manifest {
            let provenance_key = manifest::provenance_path(&filename);

            let statement = manifest::generate_statement(
                &filename,
                &template_hash,
                &[(
                    format!("templates/init/{filename}"),
                    manifest::content_sha256(template_content),
                )],
                env!("CARGO_PKG_NAME"),
                &format!("{}/init/v1", env!("CARGO_PKG_REPOSITORY")),
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_REPOSITORY"),
            );

            let provenance_path = module_root.join(&provenance_key);
            if let Some(parent) = provenance_path.parent() {
                fs::create_dir_all(parent).map_err(|error| {
                    Error::new(ErrorKind::Io, format!("{}: {error}", parent.display()))
                })?;
            }
            fs::write(&provenance_path, &statement).map_err(|error| {
                Error::new(
                    ErrorKind::Io,
                    format!("{}: {error}", provenance_path.display()),
                )
            })?;

            manifest_entries.insert(
                filename.to_string(),
                manifest::ManifestEntry {
                    fingerprint: template_hash,
                    provenance: Some(provenance_key),
                },
            );
        }
    }

    if !manifest_entries.is_empty() {
        let yaml = manifest::write(&manifest_entries)
            .map_err(|error| Error::new(ErrorKind::Io, format!("manifest: {error}")))?;
        fs::write(module_root.join(".manifest"), &yaml)
            .map_err(|error| Error::new(ErrorKind::Io, format!(".manifest: {error}")))?;
    }

    Ok(result)
}

fn resolve_module_name(module_root: &Path) -> String {
    module_root
        .canonicalize()
        .unwrap_or_else(|_| module_root.to_path_buf())
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests;
