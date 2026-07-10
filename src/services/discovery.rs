//! Discovery of deploy targets, local source repos, and source modules.

use super::history::extract_frontmatter_field;
use super::sidecar::parse_adoption;
use super::source::{
    parse_frontmatter, read_source_companions, resolve_sidecar, strip_frontmatter,
};
use super::target::{git_log_in_repo, sidecar_name_warning};
use crate::view::{ArtifactView, ModuleView};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn discover_targets(
    root: &Path,
    providers: &[(String, String)],
    watched_locations: &[PathBuf],
) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let home = dirs::home_dir();
    if let Some(ref home_path) = home
        && has_provider_dirs(home_path, providers)
    {
        targets.push(home_path.clone());
    }
    let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let is_home = home
        .as_ref()
        .is_some_and(|home_path| root_abs == *home_path);
    if !is_home && has_provider_dirs(&root_abs, providers) {
        targets.push(root_abs.clone());
    }
    for location in watched_locations {
        let canonical = fs::canonicalize(location).unwrap_or_else(|_| location.clone());
        if canonical != root_abs
            && !targets.contains(&canonical)
            && has_provider_dirs(&canonical, providers)
        {
            targets.push(canonical);
        }
    }
    targets
}

pub(super) fn has_provider_dirs(base: &Path, providers: &[(String, String)]) -> bool {
    providers.iter().any(|(_, dir)| base.join(dir).is_dir())
}

pub fn discover_local_repos(
    root: &Path,
    watched_locations: &[PathBuf],
) -> HashMap<String, PathBuf> {
    let mut repos = HashMap::new();
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    let mut search_dirs: Vec<PathBuf> = Vec::new();
    if let Some(parent) = canonical.parent() {
        search_dirs.push(parent.to_path_buf());
    }
    for location in watched_locations {
        let loc = fs::canonicalize(location).unwrap_or_else(|_| location.clone());
        register_repo(&loc, &mut repos);
        if let Some(parent) = loc.parent() {
            search_dirs.push(parent.to_path_buf());
        }
    }

    for search_dir in search_dirs {
        let Ok(entries) = fs::read_dir(&search_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            register_repo(&entry.path(), &mut repos);
        }
    }
    repos
}

pub(super) fn register_repo(path: &Path, repos: &mut HashMap<String, PathBuf>) {
    if !path.is_dir() || !path.join(".git").exists() {
        return;
    }
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output();
    if let Ok(output) = output
        && output.status.success()
    {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let normalized = url.trim_end_matches(".git").to_string();
        repos.insert(normalized, path.to_path_buf());
    }
}

pub(super) fn git_remote(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        None
    } else {
        Some(url.trim_end_matches(".git").to_string())
    }
}

/// Scans a forge module's source artifacts (`agents/`, `rules/`, `skills/`)
/// and their adoption provenance sidecars. Returns `None` if the directory
/// holds no source artifacts.
pub(super) fn scan_source_module(root: &Path) -> Option<ModuleView> {
    let source_uri = git_remote(root).unwrap_or_else(|| root.to_string_lossy().to_string());
    let module_name = root.file_name().map_or_else(
        || "module".to_string(),
        |name| name.to_string_lossy().to_string(),
    );

    let mut artifacts = Vec::new();
    artifacts.extend(scan_flat_kind(root, "agents"));
    artifacts.extend(scan_flat_kind(root, "rules"));
    artifacts.extend(scan_skill_kind(root));

    if artifacts.is_empty() {
        return None;
    }
    Some(ModuleView {
        name: module_name,
        version: String::new(),
        description: String::new(),
        source_uri,
        is_target: true,
        artifacts,
    })
}

fn scan_flat_kind(root: &Path, kind: &str) -> Vec<ArtifactView> {
    let kind_dir = root.join(kind);
    let Ok(entries) = fs::read_dir(&kind_dir) else {
        return Vec::new();
    };
    let mut artifacts = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };
        let relative_path = format!("{kind}/{name}.md");
        let sidecar = resolve_sidecar(&kind_dir, Path::new(&relative_path))
            .unwrap_or_else(|| kind_dir.join(".provenance").join(format!("{name}.yaml")));
        artifacts.push(build_source_artifact(
            root,
            kind,
            &name,
            &path,
            &relative_path,
            &sidecar,
        ));
    }
    artifacts
}

fn scan_skill_kind(root: &Path) -> Vec<ArtifactView> {
    let skills_root = root.join("skills");
    let Ok(entries) = fs::read_dir(&skills_root) else {
        return Vec::new();
    };
    let mut artifacts = Vec::new();
    for entry in entries.flatten() {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        let Some(name) = skill_dir
            .file_name()
            .map(|dir| dir.to_string_lossy().to_string())
        else {
            continue;
        };
        let relative_path = format!("skills/{name}/SKILL.md");
        let sidecar = resolve_sidecar(&skill_dir, Path::new(&relative_path))
            .unwrap_or_else(|| skill_dir.join(".provenance").join(format!("{name}.yaml")));
        let mut artifact =
            build_source_artifact(root, "skills", &name, &skill_file, &relative_path, &sidecar);
        artifact.companions = read_source_companions(&skill_dir, &name);
        artifacts.push(artifact);
    }
    artifacts
}

fn build_source_artifact(
    repo_root: &Path,
    kind: &str,
    name: &str,
    file_path: &Path,
    relative_path: &str,
    sidecar_path: &Path,
) -> ArtifactView {
    let raw_source = fs::read_to_string(file_path).unwrap_or_default();
    let description = extract_frontmatter_field(&raw_source, "description");
    let metadata = parse_frontmatter(&raw_source);
    let content_body = strip_frontmatter(&raw_source);
    let content_preview = if description.is_empty() {
        content_body.lines().take(10).collect::<Vec<_>>().join("\n")
    } else {
        String::new()
    };
    let adoption = fs::read_to_string(sidecar_path)
        .ok()
        .and_then(|content| parse_adoption(&content));
    let git_log = git_log_in_repo(repo_root, relative_path);
    let sidecar_warning = sidecar_name_warning(relative_path, sidecar_path);
    ArtifactView {
        name: name.to_string(),
        kind: kind.to_string(),
        module: String::new(),
        relative_path: relative_path.to_string(),
        source_path: relative_path.to_string(),
        description,
        content_preview,
        content_body,
        raw_source,
        metadata,
        providers: BTreeMap::new(),
        git_log,
        adoption,
        sidecar_warning,
        broken_refs: Vec::new(),
        age_days: None,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
        vcs: None,
    }
}

pub(super) fn module_name_from_source(source_uri: &str) -> String {
    source_uri
        .rsplit('/')
        .next()
        .unwrap_or(source_uri)
        .trim_end_matches(".git")
        .to_string()
}
