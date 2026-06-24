//! Deployed provenance sidecar collection and verification.

use super::content_kinds;
use super::history::recorded_input_sha;
use commands::manifest;
use commands::view::{ProvenanceArtifact, ProvenanceView};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn collect_provenance(
    target_base: &Path,
    providers: &[(String, String)],
    provenance: &mut Vec<ProvenanceView>,
) {
    let target_label = deployment_target_label(target_base);
    let mut by_source: BTreeMap<String, ProvenanceView> = BTreeMap::new();
    for (harness_name, provider_dir) in providers {
        let provider_path = target_base.join(provider_dir);
        if !provider_path.is_dir() {
            continue;
        }
        walk_provenance_dirs(&provider_path, harness_name, &target_label, &mut by_source);
    }
    provenance.extend(by_source.into_values());
}

/// Short label for a deployment target base: `~` for the home directory,
/// otherwise the final path component.
pub(super) fn deployment_target_label(target_base: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && target_base == home
    {
        return "~".to_string();
    }
    target_base.file_name().map_or_else(
        || target_base.to_string_lossy().to_string(),
        |name| name.to_string_lossy().to_string(),
    )
}

pub(super) fn walk_provenance_dirs(
    provider_path: &Path,
    harness_name: &str,
    target_label: &str,
    by_source: &mut BTreeMap<String, ProvenanceView>,
) {
    for content_dir in content_kinds() {
        let kind_dir = provider_path.join(content_dir);
        let prov_dirs = find_provenance_dirs(&kind_dir);
        for prov_dir in &prov_dirs {
            let Ok(entries) = fs::read_dir(prov_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
                    continue;
                }
                let Ok(sidecar_content) = fs::read_to_string(&path) else {
                    continue;
                };
                let source =
                    extract_source_uri(&sidecar_content).unwrap_or_else(|| "unknown".to_string());
                let parsed =
                    parse_sidecar(&sidecar_content, provider_path, harness_name, target_label);
                let record = by_source
                    .entry(source.clone())
                    .or_insert_with(|| ProvenanceView {
                        source_uri: source,
                        verified: 0,
                        total: 0,
                        orphans: Vec::new(),
                        artifacts: Vec::new(),
                    });
                record.total += 1;
                if parsed.verified {
                    record.verified += 1;
                }
                record.artifacts.push(parsed);
            }
        }
    }
}

pub(super) fn find_provenance_dirs(base: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let direct = base.join(".provenance");
    if direct.is_dir() {
        dirs.push(direct);
    }
    let Ok(entries) = fs::read_dir(base) else {
        return dirs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let nested = path.join(".provenance");
            if nested.is_dir() {
                dirs.push(nested);
            }
        }
    }
    dirs
}

pub(super) fn extract_source_uri(sidecar_content: &str) -> Option<String> {
    for line in sidecar_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("source:") {
            return Some(trimmed.trim_start_matches("source:").trim().to_string());
        }
    }
    None
}

pub(super) fn parse_sidecar(
    sidecar_content: &str,
    provider_path: &Path,
    harness_name: &str,
    target_label: &str,
) -> ProvenanceArtifact {
    let mut subject_name = String::new();
    let mut expected_sha = String::new();
    let mut source_path = String::new();
    for line in sidecar_content.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        if let Some(name) = trimmed.strip_prefix("name:") {
            subject_name = name.trim().to_string();
        }
        if let Some(sha) = trimmed.strip_prefix("sha256:")
            && expected_sha.is_empty()
        {
            expected_sha = sha.trim().to_string();
        }
        if let Some(uri) = trimmed.strip_prefix("uri:") {
            source_path = uri.trim().to_string();
        }
    }
    let deployed_rel = subject_name
        .split('/')
        .skip(1)
        .collect::<Vec<_>>()
        .join("/");
    let deployed_path = provider_path.join(&deployed_rel);
    let deployed_sha = fs::read_to_string(&deployed_path)
        .map(|content| manifest::content_sha256(&content))
        .unwrap_or_default();
    let verified = !expected_sha.is_empty() && deployed_sha == expected_sha;
    let input_sha = recorded_input_sha(sidecar_content).unwrap_or_default();
    ProvenanceArtifact {
        deployed_path: deployed_rel,
        source_path,
        harness: harness_name.to_string(),
        target: target_label.to_string(),
        verified,
        deployed_sha,
        expected_sha,
        input_sha,
    }
}
