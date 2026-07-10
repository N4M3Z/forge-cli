//! Scan deployment targets and build a `DashboardView`.
//!
//! Forge artifacts can be deployed anywhere: `~/.claude/` (user-scope),
//! `./.claude/` (project-scope), or a custom `--target` path. The scanner
//! reads `.manifest` files from known provider directories at each target
//! and groups artifacts by their source module (via provenance `source_uri`).

mod adr;
pub mod builders;
mod discovery;
pub mod files;
mod history;
mod provenance;
mod references;
mod sidecar;
mod source;
mod target;
mod vcs;

pub use adr::build_adr_artifact;
pub use discovery::discover_local_repos;
pub use history::{
    extract_frontmatter_field, git_log_for_artifact, read_source_adoption, read_source_sidecar,
    source_at_deploy,
};
pub use source::strip_frontmatter;

use crate::error::{Error, ErrorKind};
use crate::provider::ContentKind;
use crate::view::{DashboardView, ModuleView, StatusSummary, Variant};
use adr::discover_adrs;
use discovery::{discover_targets, scan_source_module};
use provenance::collect_provenance;
use references::artifact_staleness;
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use target::{PendingCompanion, attach_companions, scan_target};

/// Content kinds scanned, sourced from the shared `ContentKind` enum rather
/// than a local hardcoded list.
pub(super) fn content_kinds() -> [&'static str; 3] {
    [
        ContentKind::Agents.as_str(),
        ContentKind::Rules.as_str(),
        ContentKind::Skills.as_str(),
    ]
}

pub fn build_view(
    root: &Path,
    providers: &[(String, String)],
    watched_locations: &[PathBuf],
) -> Result<DashboardView, Error> {
    let targets = discover_targets(root, providers, watched_locations);
    if targets.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no deployed provider directories found at ~ or {}",
                root.display()
            ),
        ));
    }

    let local_repos = discover_local_repos(root, watched_locations);
    let mut modules_by_source: BTreeMap<String, ModuleView> = BTreeMap::new();
    let mut summary = StatusSummary::default();
    let mut pending_companions: Vec<PendingCompanion> = Vec::new();

    for target_base in &targets {
        scan_target(
            target_base,
            &mut modules_by_source,
            &mut summary,
            &local_repos,
            &mut pending_companions,
            providers,
        );
    }

    attach_companions(&mut modules_by_source, pending_companions);

    let mut modules: Vec<ModuleView> = modules_by_source.into_values().collect();

    for location in watched_locations {
        let module_root = fs::canonicalize(location).unwrap_or_else(|_| location.clone());
        if let Some(mut watched_module) = scan_source_module(&module_root) {
            watched_module.is_target = true;
            modules.push(watched_module);
        }
    }

    if modules.is_empty() {
        modules.push(ModuleView {
            name: "(no manifest)".to_string(),
            version: String::new(),
            description: "No .manifest files found at scanned targets".to_string(),
            source_uri: String::new(),
            is_target: false,
            artifacts: Vec::new(),
            local_path: None,
            vcs: None,
            git_log: Vec::new(),
        });
    }

    let mut provenance = Vec::new();
    for target_base in &targets {
        collect_provenance(target_base, providers, &mut provenance);
    }

    let provider_names: Vec<String> = providers.iter().map(|(name, _)| name.clone()).collect();
    for (module_index, module) in modules.iter_mut().enumerate() {
        module.artifacts.sort_by(|a, b| a.name.cmp(&b.name));
        let repo = local_repos.get(module.source_uri.trim_end_matches(".git"));
        let repo_vcs = repo.and_then(|repo| vcs::repo_vcs(repo));
        module.local_path = repo.cloned();
        module.vcs = repo_vcs.as_ref().map(vcs::RepoVcs::module_state);
        module.git_log = repo.map(|repo| vcs::repo_log(repo)).unwrap_or_default();
        let tint = module_index % 8;
        for artifact in &mut module.artifacts {
            artifact.module.clone_from(&module.name);
            artifact.module_tint = tint;
            let vcs_path = if artifact.source_path.is_empty() {
                &artifact.relative_path
            } else {
                &artifact.source_path
            };
            artifact.vcs = repo_vcs.as_ref().map(|state| state.state_for(vcs_path));
            let (broken, age) = artifact_staleness(
                repo,
                &artifact.relative_path,
                &artifact.raw_source,
                artifact.latest_commit_date(),
            );
            artifact.broken_refs = broken;
            artifact.age_days = age;
            artifact.variants = repo
                .map(|repo| collect_variants(repo, &artifact.relative_path, &provider_names))
                .unwrap_or_default();
        }
    }

    let adrs = discover_adrs(&local_repos, &active_repo_names(&modules, root));

    Ok(DashboardView {
        modules,
        summary,
        provenance,
        adrs,
    })
}

/// Directory-name allowlist for ADRs and schemas: the active modules plus the
/// repo the dashboard runs in. Confines both to the same source set as the
/// rest of the dashboard, dropping ADRs from unrelated sibling repos.
pub fn active_repo_names(modules: &[ModuleView], root: &Path) -> HashSet<String> {
    let mut names: HashSet<String> = modules.iter().map(|module| module.name.clone()).collect();
    if let Some(root_name) = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .file_name()
    {
        names.insert(root_name.to_string_lossy().to_string());
    }
    names
}

/// Discovers harness and model qualifier overrides of a base artifact in the
/// source tree (PROV-0005): `<kind-dir>/<provider>/<file>` for harness-level and
/// `<kind-dir>/<provider>/<model>/<file>` for model-level, plus the `user/`
/// overlay. The base directory is the artifact file's parent, so the same logic
/// serves flat kinds (rules, agents) and skill directories alike.
pub(super) fn collect_variants(
    repo: &Path,
    relative_path: &str,
    provider_names: &[String],
) -> Vec<Variant> {
    let base_file = repo.join(relative_path);
    let Some(base_dir) = base_file.parent() else {
        return Vec::new();
    };
    let Some(file_name) = base_file.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let mut qualifiers: Vec<&str> = provider_names.iter().map(String::as_str).collect();
    qualifiers.push("user");
    let mut variants = Vec::new();
    for provider in qualifiers {
        let provider_dir = base_dir.join(provider);
        if !provider_dir.is_dir() {
            continue;
        }
        let provider_file = provider_dir.join(file_name);
        if provider_file.is_file() {
            variants.push(make_variant(repo, provider, "", &provider_file));
        }
        let Ok(entries) = fs::read_dir(&provider_dir) else {
            continue;
        };
        let mut model_dirs: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .collect();
        model_dirs.sort();
        for model in model_dirs {
            let model_file = provider_dir.join(&model).join(file_name);
            if model_file.is_file() {
                variants.push(make_variant(repo, provider, &model, &model_file));
            }
        }
    }
    variants
}

pub(super) fn make_variant(repo: &Path, provider: &str, model: &str, file: &Path) -> Variant {
    let relative_path = file
        .strip_prefix(repo)
        .unwrap_or(file)
        .to_string_lossy()
        .to_string();
    let content = fs::read_to_string(file).unwrap_or_default();
    let mode = match extract_frontmatter_field(&content, "mode") {
        mode if mode.is_empty() => "replace".to_string(),
        mode => mode,
    };
    let qualifier = if model.is_empty() {
        provider.to_string()
    } else {
        format!("{provider}/{model}")
    };
    Variant {
        qualifier,
        provider: provider.to_string(),
        model: model.to_string(),
        relative_path,
        mode,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest;
    use std::fs;

    #[test]
    fn build_view_reads_deployed_skill_from_provider_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let provider_dir = temp.path().join(".claude");
        let skill_path = provider_dir.join("skills/Demo/SKILL.md");
        let sidecar_path = provider_dir.join("skills/Demo/.provenance/SKILL.yaml");
        let skill_content = "---\ndescription: Demo skill\n---\nUse the demo skill.\n";
        let fingerprint = manifest::content_sha256(skill_content);

        fs::create_dir_all(skill_path.parent().unwrap()).unwrap();
        fs::write(&skill_path, skill_content).unwrap();
        fs::create_dir_all(sidecar_path.parent().unwrap()).unwrap();
        fs::write(
            &sidecar_path,
            format!(
                "source: https://github.com/example/forge-demo.git\n\
                 subject:\n\
                 - name: skills/Demo/SKILL.md\n\
                   digest:\n\
                     sha256: {fingerprint}\n\
                 resolvedDependencies:\n\
                 - uri: skills/Demo/SKILL.md\n\
                   digest:\n\
                     sha256: {fingerprint}\n"
            ),
        )
        .unwrap();
        fs::write(
            provider_dir.join(".manifest"),
            format!(
                "skills:\n  Demo:\n    SKILL.md:\n      fingerprint: {fingerprint}\n      provenance: skills/Demo/.provenance/SKILL.yaml\n"
            ),
        )
        .unwrap();

        let providers = vec![("claude".to_string(), ".claude".to_string())];
        let view = build_view(temp.path(), &providers, &[]).unwrap();
        let module = view
            .modules
            .iter()
            .find(|module| module.name == "forge-demo")
            .unwrap();
        let artifact = module
            .artifacts
            .iter()
            .find(|artifact| artifact.name == "Demo" && artifact.kind == "skills")
            .unwrap();

        assert_eq!(artifact.description, "Demo skill");
        assert_eq!(artifact.content_body, "Use the demo skill.\n");
        assert_eq!(
            artifact.providers.get("claude").unwrap().status,
            manifest::FileStatus::Unchanged
        );
    }
}
