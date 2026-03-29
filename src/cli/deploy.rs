use commands::error::{Error, ErrorKind};
use commands::manifest;
use commands::result::{ActionResult, DeployedFile, PrunedFile, SkipReason, SkippedFile};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use crate::cli::config;

/// Copy assembled files from build/ to provider target directories.
///
/// Reads `.manifest` from each provider's target to detect user modifications.
/// After copying, writes an updated `.manifest` recording what was deployed.
///
/// ```text
/// New       → copy
/// Unchanged → skip
/// Stale     → copy (source changed since last build)
/// Modified  → skip (unless --force)
/// ```
pub fn execute(
    path: &str,
    target: Option<&str>,
    force: bool,
    prune: bool,
    _interactive: bool,
) -> Result<ActionResult, Error> {
    let module_root = Path::new(path);
    let mut result = ActionResult::new();

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;

    for (provider_name, provider_config) in &providers {
        let build_provider_dir = module_root.join("build").join(provider_name);
        if !build_provider_dir.is_dir() {
            continue;
        }

        let target_base = match target {
            Some(dir) => Path::new(dir).join(&provider_config.target),
            None => Path::new(&provider_config.target).to_path_buf(),
        };

        if let Some(dir) = target {
            validate_target_boundary(&target_base, Path::new(dir))?;
        }

        let mut existing_manifest = load_deployed_manifest(&target_base);
        let mut deployed_keys: HashSet<String> = HashSet::new();

        deploy_provider_files(
            &build_provider_dir,
            &target_base,
            &mut existing_manifest,
            &mut deployed_keys,
            &mut result,
            provider_name,
            force,
        )?;

        // Stale detection only with --prune (shared manifests across
        // modules make automatic detection unreliable without module ownership)
        if prune {
            let stale_keys: Vec<String> = existing_manifest
                .keys()
                .filter(|key| !deployed_keys.contains(*key))
                .filter(|key| {
                    ["agents/", "skills/", "rules/"]
                        .iter()
                        .any(|prefix| key.starts_with(prefix))
                })
                .cloned()
                .collect();

            for stale_key in &stale_keys {
                let stale_path = target_base.join(stale_key);
                if stale_path.is_file() {
                    let _ = fs::remove_file(&stale_path);
                }
                existing_manifest.remove(stale_key);
                result.pruned.push(PrunedFile {
                    target: stale_path.to_string_lossy().to_string(),
                    provider: provider_name.to_owned(),
                });
            }
        }

        write_manifest(&target_base, &existing_manifest)?;
    }

    Ok(result)
}

/// Deploy all content kinds (agents, skills, rules) for a single provider.
fn deploy_provider_files(
    build_provider_dir: &Path,
    target_base: &Path,
    new_manifest: &mut HashMap<String, manifest::ManifestEntry>,
    deployed_keys: &mut HashSet<String>,
    result: &mut ActionResult,
    provider_name: &str,
    force: bool,
) -> Result<(), Error> {
    for kind in &["agents", "skills", "rules"] {
        let kind_dir = build_provider_dir.join(kind);
        if !kind_dir.is_dir() {
            continue;
        }

        let files = collect_files_recursive(&kind_dir)?;

        for build_path in files {
            if build_path.extension().unwrap_or_default() == "yaml" {
                continue;
            }

            let relative = build_path
                .strip_prefix(&kind_dir)
                .unwrap_or(&build_path)
                .to_string_lossy()
                .to_string();
            let manifest_key = format!("{kind}/{relative}");
            deployed_keys.insert(manifest_key.clone());
            let target_path = target_base.join(kind).join(&relative);

            let build_content = config::read_file(&build_path)?;
            let build_sha256 = manifest::content_sha256(&build_content);

            let target_content = fs::read_to_string(&target_path).ok();
            let status = manifest::status(
                target_content.as_deref(),
                new_manifest.get(&manifest_key),
                &build_sha256,
            );

            match status {
                manifest::FileStatus::New | manifest::FileStatus::Stale => {
                    copy_file(&build_path, &target_path)?;
                    new_manifest.insert(
                        manifest_key,
                        manifest::ManifestEntry {
                            sha256: build_sha256,
                        },
                    );
                    result.installed.push(DeployedFile {
                        source: build_path.to_string_lossy().to_string(),
                        target: target_path.to_string_lossy().to_string(),
                        provider: provider_name.to_owned(),
                    });
                }
                manifest::FileStatus::Unchanged => {
                    new_manifest.insert(
                        manifest_key,
                        manifest::ManifestEntry {
                            sha256: build_sha256,
                        },
                    );
                    result.skipped.push(SkippedFile {
                        target: target_path.to_string_lossy().to_string(),
                        provider: provider_name.to_owned(),
                        reason: SkipReason::Unchanged,
                    });
                }
                manifest::FileStatus::Modified => {
                    if force {
                        copy_file(&build_path, &target_path)?;
                        new_manifest.insert(
                            manifest_key,
                            manifest::ManifestEntry {
                                sha256: build_sha256,
                            },
                        );
                        result.installed.push(DeployedFile {
                            source: build_path.to_string_lossy().to_string(),
                            target: target_path.to_string_lossy().to_string(),
                            provider: provider_name.to_owned(),
                        });
                    } else {
                        result.skipped.push(SkippedFile {
                            target: target_path.to_string_lossy().to_string(),
                            provider: provider_name.to_owned(),
                            reason: SkipReason::UserModified,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// Verify the resolved target path stays within the specified base directory.
fn validate_target_boundary(target_path: &Path, base_directory: &Path) -> Result<(), Error> {
    let resolved_target = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.to_path_buf());
    let resolved_base = base_directory
        .canonicalize()
        .unwrap_or_else(|_| base_directory.to_path_buf());

    if !resolved_target.starts_with(&resolved_base) {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "target path escapes base directory: {} resolves outside {}",
                target_path.display(),
                resolved_base.display()
            ),
        ));
    }
    Ok(())
}

/// Load the previously deployed `.manifest` from a provider's target directory.
fn load_deployed_manifest(target_base: &Path) -> HashMap<String, manifest::ManifestEntry> {
    let manifest_path = target_base.join(".manifest");
    let Ok(content) = fs::read_to_string(&manifest_path) else {
        return HashMap::new();
    };
    manifest::read(&content).unwrap_or_default()
}

/// Write `.manifest` to the provider's target directory after deployment.
fn write_manifest(
    target_base: &Path,
    entries: &HashMap<String, manifest::ManifestEntry>,
) -> Result<(), Error> {
    let yaml = manifest::write(entries)
        .map_err(|e| Error::new(ErrorKind::Io, format!("failed to serialize manifest: {e}")))?;

    fs::create_dir_all(target_base).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {e}", target_base.display()),
        )
    })?;

    let manifest_path = target_base.join(".manifest");
    fs::write(&manifest_path, &yaml)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot write .manifest: {e}")))
}

/// Copy a file, creating parent directories as needed.
fn copy_file(source: &Path, target: &Path) -> Result<(), Error> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            Error::new(
                ErrorKind::Io,
                format!("cannot create {}: {e}", parent.display()),
            )
        })?;
    }
    fs::copy(source, target).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!(
                "cannot copy {} -> {}: {e}",
                source.display(),
                target.display()
            ),
        )
    })?;
    Ok(())
}

/// Recursively collect all files in a directory.
fn collect_files_recursive(dir: &Path) -> Result<Vec<std::path::PathBuf>, Error> {
    let mut files = Vec::new();

    let entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;

        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_files_recursive(&path)?);
        } else {
            files.push(path);
        }
    }

    Ok(files)
}
