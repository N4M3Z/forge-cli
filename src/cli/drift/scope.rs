//! Manifest-scoped drift verification: compare a module's assembled `build/`
//! against where it was deployed, limited to this module's own files.
//!
//! `forge drift --upstream <DIR>` compares two module trees by name and, run
//! against a multi-module deployment like `~/.claude`, floods the report with
//! every other module's files as "upstream only". This mode instead walks
//! `build/<provider>` (this module's output) and compares each file to its
//! deployed counterpart, then consults the deployed `.manifest` + provenance
//! attribution to flag this module's deployed files that are no longer built.

use commands::error::{Error, ErrorKind};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::Path;

use super::{DriftEntry, DriftResult, DriftStatus, compare_file_content, print_drift_result};
use crate::cli::config;
use crate::cli::deploy::{is_owned_by_module, load_deployed_manifest};
use commands::provider::{ContentKind, ProviderConfig};

const CONTENT_PREFIXES: [&str; 3] = ["agents/", "skills/", "rules/"];

/// Verify each provider's `build/<provider>` against `<target_base>/<provider
/// target>` (mirroring `forge install --target`), scoped to this module.
pub fn execute(
    source: &str,
    target_base: &str,
    ignore: &[String],
    json_output: bool,
) -> Result<i32, Error> {
    let module_root = Path::new(source);
    if !module_root.join("module.yaml").is_file() {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "{source} is not a module root (no module.yaml); --target verify compares its build/ against a deployment"
            ),
        ));
    }

    let source_uri = config::load_source_uri(module_root);
    let module_name = (!source_uri.is_empty()).then_some(source_uri.as_str());

    let merged_config = config::load_merged_config(module_root)?;
    let providers = config::load_providers(&merged_config)?;

    let ignored: HashSet<&str> = ignore.iter().map(String::as_str).collect();
    let base = Path::new(target_base);
    let mut result = DriftResult::default();
    let mut compared_any = false;

    for (provider_name, provider_config) in &providers {
        let build_dir = module_root.join("build").join(provider_name);
        if !build_dir.is_dir() {
            continue;
        }
        compared_any = true;
        compare_provider(
            &mut result,
            &build_dir,
            base,
            provider_config,
            provider_name,
            module_name,
            &ignored,
        );
    }

    if !compared_any {
        return Err(Error::new(
            ErrorKind::Io,
            format!(
                "no build/ output under {source}; run `forge assemble` or `forge install` first"
            ),
        ));
    }

    if json_output {
        match serde_json::to_string_pretty(&result) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize drift result: {error}"),
        }
    } else {
        print_drift_result(&result);
    }

    let has_drift = result.entries.iter().any(|entry| {
        matches!(
            entry.status,
            DriftStatus::FrontmatterOnly
                | DriftStatus::BodyOnly
                | DriftStatus::Both
                | DriftStatus::UpstreamOnly
        )
    });
    Ok(i32::from(has_drift))
}

fn compare_provider(
    result: &mut DriftResult,
    build_dir: &Path,
    target_base: &Path,
    provider_config: &ProviderConfig,
    provider_name: &str,
    module_name: Option<&str>,
    ignored: &HashSet<&str>,
) {
    let build_files = collect_content_files(build_dir);

    for (relative, build_content) in &build_files {
        let Some(kind) = kind_for_relative(relative) else {
            continue;
        };
        let deployed_base = target_base.join(provider_config.target_for_kind(kind));
        let deployed_path = deployed_base.join(relative);
        match fs::read_to_string(&deployed_path) {
            Ok(deployed_content) => {
                result.entries.push(compare_file_content(
                    relative,
                    build_content,
                    &deployed_content,
                    provider_name,
                    ignored,
                ));
            }
            Err(_) => {
                result
                    .entries
                    .push(only_entry(relative, DriftStatus::LocalOnly, provider_name));
            }
        }
    }

    // This module's deployed files (per the target manifest + provenance) that
    // are no longer built — stale deployments that should be pruned.
    for target_root in provider_config.target_roots() {
        let deployed_base = target_base.join(target_root);
        for (key, entry) in load_deployed_manifest(&deployed_base).unwrap_or_else(|error| {
            eprintln!("warning: {error}; treating manifest as empty for drift");
            std::collections::HashMap::new()
        }) {
            if !is_content_key(&key) {
                continue;
            }
            if let Some(kind) = kind_for_relative(&key) {
                let expected_base = target_base.join(provider_config.target_for_kind(kind));
                if expected_base == deployed_base && build_files.contains_key(&key) {
                    continue;
                }
            }
            if is_owned_by_module(&entry, &deployed_base, module_name) {
                result
                    .entries
                    .push(only_entry(&key, DriftStatus::UpstreamOnly, provider_name));
            }
        }
    }
}

fn only_entry(name: &str, status: DriftStatus, category: &str) -> DriftEntry {
    DriftEntry {
        name: name.to_string(),
        status,
        category: category.to_string(),
        changed_keys: Vec::new(),
        renamed_from: None,
        source_uri: None,
    }
}

fn is_content_key(key: &str) -> bool {
    CONTENT_PREFIXES
        .iter()
        .any(|prefix| key.starts_with(prefix))
}

fn kind_for_relative(relative: &str) -> Option<ContentKind> {
    match relative.split_once('/').map(|(kind, _)| kind) {
        Some("agents") => Some(ContentKind::Agents),
        Some("skills") => Some(ContentKind::Skills),
        Some("rules") => Some(ContentKind::Rules),
        _ => None,
    }
}

/// Collect content files under a provider build directory, keyed by their path
/// relative to it. Sidecar directories, `.manifest`, and dotfiles are skipped.
fn collect_content_files(build_dir: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();
    collect_recursive(build_dir, build_dir, &mut files);
    files
}

fn collect_recursive(base: &Path, current: &Path, files: &mut BTreeMap<String, String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if path.is_dir() {
            collect_recursive(base, &path, files);
        } else if let Ok(content) = fs::read_to_string(&path) {
            let relative = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            files.insert(relative, content);
        }
    }
}

#[cfg(test)]
mod tests;
