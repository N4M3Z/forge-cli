use commands::error::{Error, ErrorKind};
use commands::manifest;
use commands::result::{ActionResult, DeployedFile, PrunedFile, SkipReason, SkippedFile};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

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
#[allow(clippy::fn_params_excessive_bools, clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub fn execute(
    path: &str,
    target: Option<&str>,
    requested_providers: &[String],
    force: bool,
    prune: bool,
    _interactive: bool,
    dry_run: bool,
    only: Option<&str>,
) -> Result<ActionResult, Error> {
    // A scoped deploy leaves everything else at the target alone: pruning
    // against a filtered key set would quarantine the rest of the module.
    let prune = prune && only.is_none();
    let module_root = Path::new(path);
    require_module_root(module_root)?;
    let mut result = ActionResult::new();

    let merged_config = config::load_merged_config(module_root)?;
    let mut providers = config::load_providers(&merged_config)?;

    if !requested_providers.is_empty() {
        providers = filter_requested_providers(&providers, requested_providers)?;
    }

    let module_source_uri = config::load_source_uri(module_root);
    let module_name = if module_source_uri.is_empty() {
        None
    } else {
        Some(module_source_uri)
    };

    // Consumer mode (.forge present): the consumer dir IS the target the user wants
    // provider trees written into, so an omitted --target defaults to --source.
    let effective_target: Option<&str> = match target {
        Some(dir) => Some(dir),
        None if module_root.join(".forge").is_file() => Some(path),
        None => None,
    };

    for (provider_name, provider_config) in &providers {
        let build_provider_dir = module_root.join("build").join(provider_name);
        if !build_provider_dir.is_dir() {
            continue;
        }

        let mut manifests: HashMap<PathBuf, HashMap<String, manifest::ManifestEntry>> =
            HashMap::new();
        let mut deployed_by_root: HashMap<PathBuf, HashSet<String>> = HashMap::new();

        for target_root in provider_config.target_roots() {
            let target_base = resolve_target_base(target_root, effective_target);
            if let Some(dir) = effective_target {
                validate_target_boundary(&target_base, Path::new(dir))?;
            }
            deployed_by_root.entry(target_base.clone()).or_default();
            manifests
                .entry(target_base.clone())
                .or_insert_with(|| load_deployed_manifest(&target_base));
        }

        for kind in commands::provider::ContentKind::ALL {
            let kind_dir = build_provider_dir.join(kind.as_str());
            if !kind_dir.is_dir() {
                continue;
            }

            let target_base =
                resolve_target_base(provider_config.target_for_kind(*kind), effective_target);
            if let Some(dir) = effective_target {
                validate_target_boundary(&target_base, Path::new(dir))?;
            }

            let existing_manifest = manifests
                .entry(target_base.clone())
                .or_insert_with(|| load_deployed_manifest(&target_base));
            let deployed_keys = deployed_by_root.entry(target_base.clone()).or_default();

            deploy_provider_kind_files(
                &kind_dir,
                *kind,
                &target_base,
                existing_manifest,
                deployed_keys,
                &mut result,
                provider_name,
                force,
                only,
            )?;
        }

        for (target_base, mut existing_manifest) in manifests {
            let deployed_keys = deployed_by_root.remove(&target_base).unwrap_or_default();
            if prune {
                prune_stale_files(
                    &target_base,
                    &mut existing_manifest,
                    &deployed_keys,
                    &mut result,
                    provider_name,
                    module_name.as_deref(),
                    force,
                    dry_run,
                );
            }
            write_manifest(&target_base, &existing_manifest)?;
        }
    }

    Ok(result)
}

fn resolve_target_base(target_root: &str, effective_target: Option<&str>) -> PathBuf {
    match effective_target {
        Some(dir) => Path::new(dir).join(target_root),
        None => Path::new(target_root).to_path_buf(),
    }
}

/// Deploy one content kind for a single provider.
#[allow(clippy::too_many_arguments)]
fn deploy_provider_kind_files(
    kind_dir: &Path,
    kind: commands::provider::ContentKind,
    target_base: &Path,
    new_manifest: &mut HashMap<String, manifest::ManifestEntry>,
    deployed_keys: &mut HashSet<String>,
    result: &mut ActionResult,
    provider_name: &str,
    force: bool,
    only: Option<&str>,
) -> Result<(), Error> {
    let files = collect_files_recursive(kind_dir)?;

    for build_path in files {
        if build_path.extension().unwrap_or_default() == "yaml" {
            continue;
        }

        let relative = build_path
            .strip_prefix(kind_dir)
            .unwrap_or(&build_path)
            .to_string_lossy()
            .to_string();
        let manifest_key = format!("{kind}/{relative}");
        if only.is_some_and(|prefix| !manifest_key.starts_with(prefix)) {
            continue;
        }
        deployed_keys.insert(manifest_key.clone());
        let target_path = target_base.join(kind.as_str()).join(&relative);

        let build_content = config::read_file(&build_path)?;
        let build_fingerprint = manifest::content_sha256(&build_content);
        let provenance_relative = manifest::provenance_path(&manifest_key);
        let sidecar_source = manifest::sidecar_path(&build_path);

        if sidecar_source.is_file() {
            let provenance_target = target_base.join(&provenance_relative);
            let _ = copy_file(&sidecar_source, &provenance_target);
        }

        let target_content = fs::read_to_string(&target_path).ok();
        let status = manifest::status(
            target_content.as_deref(),
            new_manifest.get(&manifest_key),
            &build_fingerprint,
        );

        match status {
            manifest::FileStatus::New | manifest::FileStatus::Stale => {
                copy_file(&build_path, &target_path)?;
                new_manifest.insert(
                    manifest_key,
                    manifest::ManifestEntry {
                        fingerprint: build_fingerprint.clone(),
                        provenance: Some(provenance_relative.clone()),
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
                        fingerprint: build_fingerprint.clone(),
                        provenance: Some(provenance_relative.clone()),
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
                            fingerprint: build_fingerprint.clone(),
                            provenance: Some(provenance_relative.clone()),
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
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
fn prune_stale_files(
    target_base: &Path,
    existing_manifest: &mut HashMap<String, manifest::ManifestEntry>,
    deployed_keys: &HashSet<String>,
    result: &mut ActionResult,
    provider_name: &str,
    module_name: Option<&str>,
    force: bool,
    dry_run: bool,
) {
    let stale_keys: Vec<String> = existing_manifest
        .iter()
        .filter(|(key, _)| !deployed_keys.contains(*key))
        .filter(|(key, _)| {
            ["agents/", "skills/", "rules/"]
                .iter()
                .any(|prefix| key.starts_with(prefix))
        })
        .filter(|(_, entry)| is_owned_by_module(entry, target_base, module_name))
        .map(|(key, _)| key.clone())
        .collect();

    if stale_keys.is_empty() {
        return;
    }

    let stamp = chrono::Utc::now().format("%Y-%m-%d-%H%MZ").to_string();
    let trash_root = target_base.join(".trash").join(&stamp);
    let mut skipped_modified = 0;

    for stale_key in &stale_keys {
        let stale_path = target_base.join(stale_key);
        let trash_dest = trash_root.join(stale_key);

        // Refuse to prune a file whose on-disk content no longer matches the
        // recorded fingerprint: that signals local edits the user might want
        // to keep. --force overrides both deploy and prune protection.
        if !force
            && stale_path.is_file()
            && let Some(expected) = existing_manifest
                .get(stale_key)
                .map(|entry| entry.fingerprint.clone())
            && let Ok(current) = fs::read_to_string(&stale_path)
            && manifest::content_sha256(&current) != expected
        {
            eprintln!(
                "forge prune: skipping {} (modified locally; pass --force to prune)",
                stale_path.display()
            );
            skipped_modified += 1;
            continue;
        }

        if dry_run {
            eprintln!(
                "forge prune: would move {} -> {}",
                stale_path.display(),
                trash_dest.display()
            );
            continue;
        }

        if let Some(parent) = trash_dest.parent()
            && let Err(error) = fs::create_dir_all(parent)
        {
            eprintln!(
                "warning: cannot create quarantine dir {}: {error}",
                parent.display()
            );
            continue;
        }

        if stale_path.is_file() {
            if let Err(error) = fs::rename(&stale_path, &trash_dest) {
                eprintln!(
                    "warning: cannot quarantine {}: {error}",
                    stale_path.display()
                );
                continue;
            }
            prune_empty_parents(stale_path.parent(), target_base);
        }

        let provenance_rel = manifest::provenance_path(stale_key);
        let provenance_path = target_base.join(&provenance_rel);
        if provenance_path.is_file() {
            let provenance_trash = trash_root.join(&provenance_rel);
            if let Some(parent) = provenance_trash.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::rename(&provenance_path, &provenance_trash);
            prune_empty_parents(provenance_path.parent(), target_base);
        }

        existing_manifest.remove(stale_key);
        result.pruned.push(PrunedFile {
            target: stale_path.to_string_lossy().to_string(),
            provider: provider_name.to_owned(),
        });
    }

    let action = if dry_run { "would move" } else { "moved" };
    let pruned_count = stale_keys.len() - skipped_modified;
    if pruned_count > 0 {
        let entry_label = if pruned_count == 1 {
            "entry"
        } else {
            "entries"
        };
        eprintln!(
            "forge prune: {action} {pruned_count} stale {entry_label} to {}/.trash/{}/; recoverable via mv",
            target_base.display(),
            stamp
        );
    }
    if skipped_modified > 0 {
        let entry_label = if skipped_modified == 1 {
            "entry"
        } else {
            "entries"
        };
        eprintln!(
            "forge prune: skipped {skipped_modified} modified {entry_label}; pass --force to prune"
        );
    }
}

/// Keep only the provider entries the user requested. Each requested name is
/// matched against provider keys, target directories, and aliases (the same
/// rules `ProviderConfig::matches_target` uses elsewhere). Unknown names
/// produce a single error listing all available choices.
fn filter_requested_providers(
    providers: &HashMap<String, commands::provider::ProviderConfig>,
    requested: &[String],
) -> Result<HashMap<String, commands::provider::ProviderConfig>, Error> {
    let mut matched: HashMap<String, commands::provider::ProviderConfig> = HashMap::new();
    let mut unknown: Vec<String> = Vec::new();

    for requested_name in requested {
        let hit = providers
            .iter()
            .find(|(key, config)| config.matches_target(requested_name, key));

        match hit {
            Some((key, config)) => {
                matched.entry(key.clone()).or_insert_with(|| config.clone());
            }
            None => unknown.push(requested_name.clone()),
        }
    }

    if !unknown.is_empty() {
        let mut available: Vec<&String> = providers.keys().collect();
        available.sort();
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "unknown provider(s): {}. Available: {}",
                unknown.join(", "),
                available
                    .into_iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ));
    }

    Ok(matched)
}

/// Refuse to operate on a path that isn't a forge module root or a consumer
/// repo. A consumer repo (one with `.forge`) is a valid `--source` for
/// install and deploy; the assemble step has already turned its manifest
/// into a `Vec<SourceFile>` by the time deploy runs.
fn require_module_root(module_root: &Path) -> Result<(), Error> {
    if !module_root.is_dir() {
        return Err(Error::new(
            ErrorKind::Io,
            format!("source directory not found: {}", module_root.display()),
        ));
    }
    if !module_root.join("module.yaml").is_file() && !module_root.join(".forge").is_file() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no module.yaml or .forge at {}; --source must point to a module root or consumer repo",
                module_root.display()
            ),
        ));
    }
    Ok(())
}

/// Verify the resolved target path stays within the specified base directory.
fn validate_target_boundary(target_path: &Path, base_directory: &Path) -> Result<(), Error> {
    fs::create_dir_all(target_path).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create {}: {error}", target_path.display()),
        )
    })?;

    let resolved_target = target_path.canonicalize().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot resolve {}: {error}", target_path.display()),
        )
    })?;
    let resolved_base = base_directory.canonicalize().map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot resolve {}: {error}", base_directory.display()),
        )
    })?;

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
pub(crate) fn load_deployed_manifest(
    target_base: &Path,
) -> HashMap<String, manifest::ManifestEntry> {
    let manifest_path = target_base.join(".manifest");
    let Ok(content) = fs::read_to_string(&manifest_path) else {
        return HashMap::new();
    };
    match manifest::read(&content) {
        Ok(entries) => entries,
        Err(error) => {
            eprintln!(
                "warning: corrupt .manifest at {}: {error}",
                manifest_path.display()
            );
            HashMap::new()
        }
    }
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

/// Check if a stale manifest entry was installed by the current module.
///
/// Reads the provenance sidecar and compares the recorded source URI to the
/// current module's identity. If no provenance exists or can't be read,
/// assumes ownership (prune it).
///
/// Matching is structured: both source URIs are parsed into `(host, owner, repo)`
/// tuples and compared as tuples. Bare-name equality is used only when both
/// inputs fail to parse as URLs. This prevents two modules named `forge-core`
/// at different repositories from incorrectly pruning each other's deployed
/// files, and stops `Prompts` from matching `PublishPrompts` via a substring.
pub(crate) fn is_owned_by_module(
    entry: &manifest::ManifestEntry,
    target_base: &Path,
    module_name: Option<&str>,
) -> bool {
    let Some(module) = module_name else {
        return true;
    };

    let Some(provenance_relative) = &entry.provenance else {
        return true;
    };

    let provenance_path = target_base.join(provenance_relative);
    let Ok(sidecar) = manifest::provenance::read(&provenance_path) else {
        return true;
    };

    let source_uri = &sidecar
        .provenance
        .predicate
        .build_definition
        .external_parameters
        .source;

    match (parse_repo(source_uri), parse_repo(module)) {
        (Some(a), Some(b)) => a == b,
        (None, None) => source_uri == module,
        _ => false,
    }
}

/// Remove empty parent directories walking up from `start` toward `stop`.
///
/// Stops as soon as a non-empty directory is encountered or the walk reaches
/// `stop` (or escapes the `stop` subtree). The `stop` directory itself is
/// never removed, so this cannot delete the provider target root.
fn prune_empty_parents(start: Option<&Path>, stop: &Path) {
    let mut current = start;
    while let Some(directory) = current {
        if directory == stop || !directory.starts_with(stop) {
            break;
        }
        let is_empty = fs::read_dir(directory).is_ok_and(|mut iter| iter.next().is_none());
        if !is_empty {
            break;
        }
        if fs::remove_dir(directory).is_err() {
            break;
        }
        current = directory.parent();
    }
}

/// Parse a GitHub-style repository URI into `(host, owner, repo)`.
///
/// Accepts `https://host/owner/repo`, `https://host/owner/repo.git`,
/// `git@host:owner/repo`, `git@host:owner/repo.git`, with or without a
/// trailing slash. Returns `None` for bare-name strings, which the caller
/// then compares with literal equality.
fn parse_repo(s: &str) -> Option<(String, String, String)> {
    static REPO_RE: OnceLock<Regex> = OnceLock::new();
    let re = REPO_RE.get_or_init(|| {
        Regex::new(r"^(?:https?://|git@)([^/:]+)[:/]([^/]+)/([^/.]+?)(?:\.git)?/?$")
            .expect("repo regex compiles")
    });
    let captures = re.captures(s)?;
    Some((
        captures.get(1)?.as_str().to_string(),
        captures.get(2)?.as_str().to_string(),
        captures.get(3)?.as_str().to_string(),
    ))
}

#[cfg(test)]
mod tests;
