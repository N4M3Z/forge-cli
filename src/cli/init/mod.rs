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
        let target_path = module_root.join(filename.as_ref());
        if target_path.exists() {
            result.skipped.push(SkippedFile {
                target: filename.to_string(),
                provider: "init".to_string(),
                reason: SkipReason::Unchanged,
            });
            continue;
        }

        let Some(data) = InitTemplates::get(&filename) else {
            continue;
        };
        let content = std::str::from_utf8(data.data.as_ref())
            .map_err(|error| Error::new(ErrorKind::Io, format!("{filename}: {error}")))?
            .replace("${MODULE_NAME}", &module_name)
            .replace("${VERSION}", "0.1.0");

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                Error::new(ErrorKind::Io, format!("{}: {error}", parent.display()))
            })?;
        }
        fs::write(&target_path, &content).map_err(|error| {
            Error::new(ErrorKind::Io, format!("{}: {error}", target_path.display()))
        })?;

        manifest_entries.insert(
            filename.to_string(),
            manifest::ManifestEntry {
                fingerprint: manifest::content_sha256(&content),
                provenance: None,
            },
        );
        result.installed.push(DeployedFile {
            source: format!("templates/init/{filename}"),
            target: filename.to_string(),
            provider: "init".to_string(),
        });
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
