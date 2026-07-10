//! Architecture Decision Record discovery and synthetic artifact construction.

use super::history::{extract_frontmatter_field, git_log_for_artifact, read_source_adoption};
use super::references::{artifact_staleness, truncate_summary};
use super::sidecar::{parse_adoption, recorded_subject_sha};
use super::source::{read_source_content, resolve_sidecar, strip_frontmatter};
use super::target::sidecar_name_warning;
use crate::manifest;
use crate::view::{Adr, ArtifactView};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Scans `docs/decisions/*.md` in the allowed repos for architecture decision
/// records. The filename `<ID> <Title>.md` yields id + title; status is read
/// from frontmatter when present. Repos are visited in path order so the
/// grouping is stable.
pub(super) fn discover_adrs(
    local_repos: &HashMap<String, PathBuf>,
    allowed: &HashSet<String>,
) -> Vec<Adr> {
    let mut repos: Vec<(&String, &PathBuf)> = local_repos.iter().collect();
    repos.sort_by(|a, b| a.1.cmp(b.1));
    repos.retain(|(_, path)| {
        path.file_name()
            .is_some_and(|name| allowed.contains(name.to_string_lossy().as_ref()))
    });
    let mut adrs = Vec::new();
    for (source_uri, repo_path) in repos {
        let decisions = repo_path.join("docs/decisions");
        let Ok(entries) = fs::read_dir(&decisions) else {
            continue;
        };
        let repo_name = repo_path.file_name().map_or_else(
            || source_uri.clone(),
            |name| name.to_string_lossy().to_string(),
        );
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .filter(|name| {
                !name.starts_with('.')
                    && Path::new(name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            })
            .collect();
        names.sort();
        for name in names {
            let stem = name.trim_end_matches(".md");
            let (id, title) = stem.split_once(' ').unwrap_or((stem, ""));
            let relative_path = format!("docs/decisions/{name}");
            let raw = fs::read_to_string(decisions.join(&name)).unwrap_or_default();
            let sidecar = resolve_sidecar(&decisions, Path::new(&relative_path))
                .and_then(|path| fs::read_to_string(path).ok());
            let (state, source) = adr_state(sidecar.as_deref(), &raw);
            adrs.push(Adr {
                id: id.to_string(),
                title: title.to_string(),
                status: extract_frontmatter_field(&raw, "status"),
                repo: repo_name.clone(),
                source_uri: source_uri.clone(),
                relative_path,
                state,
                source,
                summary: adr_summary(&raw),
                local_path: decisions.join(&name).to_string_lossy().into_owned(),
            });
        }
    }
    adrs
}

/// Classifies an ADR from its sidecar: `authored` (no sidecar), `modified` (the
/// copy was edited since adoption), or `copied` (still matches what was copied).
/// Also returns the copied-from source label.
fn adr_state(sidecar: Option<&str>, current: &str) -> (String, String) {
    let Some(content) = sidecar else {
        return ("authored".to_string(), String::new());
    };
    let source = parse_adoption(content)
        .map(|adoption| adoption.source_label)
        .unwrap_or_default();
    let modified =
        recorded_subject_sha(content).is_some_and(|sha| manifest::content_sha256(current) != sha);
    let state = if modified { "modified" } else { "copied" };
    (state.to_string(), source)
}

/// One-paragraph preview for the ADR list: the `## Context` section's first
/// prose paragraph when present, otherwise the first prose paragraph after the
/// title. Headings and blank lines are skipped, backticks dropped, and the
/// result is truncated at a word boundary.
fn adr_summary(raw: &str) -> String {
    let body = strip_frontmatter(raw);
    let lines: Vec<&str> = body.lines().collect();
    let start = lines
        .iter()
        .position(|line| {
            line.starts_with("##")
                && line
                    .trim_start_matches('#')
                    .trim()
                    .eq_ignore_ascii_case("context")
        })
        .map_or(0, |index| index + 1);
    let mut paragraph = String::new();
    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }
    truncate_summary(&paragraph.replace('`', ""), 260)
}

/// Builds a synthetic `ArtifactView` for an ADR so the artifact detail view can
/// render its content, frontmatter, git history, and any provenance sidecar.
/// ADRs are authored, not deployed, so providers and companions stay empty.
pub fn build_adr_artifact(adr: &Adr, local_repos: &HashMap<String, PathBuf>) -> ArtifactView {
    let content = read_source_content(&adr.source_uri, Some(&adr.relative_path), local_repos);
    let sidecar_warning = local_repos
        .get(adr.source_uri.trim_end_matches(".git"))
        .and_then(|repo| {
            let relative = Path::new(&adr.relative_path);
            let parent = repo.join(relative.parent()?);
            let sidecar = resolve_sidecar(&parent, relative)?;
            Some(sidecar_name_warning(&adr.relative_path, &sidecar))
        })
        .unwrap_or_default();
    let git_log = git_log_for_artifact(&adr.source_uri, Some(&adr.relative_path), local_repos);
    let latest_date = git_log.first().map_or("", |commit| commit.date.as_str());
    let (broken_refs, age_days) = artifact_staleness(
        local_repos.get(adr.source_uri.trim_end_matches(".git")),
        &adr.relative_path,
        &content.raw,
        latest_date,
    );
    ArtifactView {
        name: adr.id.clone(),
        kind: "adr".to_string(),
        module: adr.repo.clone(),
        relative_path: adr.relative_path.clone(),
        source_path: adr.relative_path.clone(),
        description: adr.title.clone(),
        content_preview: String::new(),
        content_body: content.body,
        raw_source: content.raw,
        metadata: content.metadata,
        providers: BTreeMap::new(),
        git_log,
        adoption: read_source_adoption(&adr.source_uri, Some(&adr.relative_path), local_repos),
        sidecar_warning,
        broken_refs,
        age_days,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
        vcs: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adr_state_classifies_authored_copied_modified() {
        assert_eq!(adr_state(None, "anything").0, "authored");
        let content = "decision body\n";
        let sha = manifest::content_sha256(content);
        let sidecar = format!(
            "provenance:\n    subject:\n        - digest:\n              sha256: {sha}\n    predicate:\n        buildDefinition:\n            buildType: https://github.com/N4M3Z/forge-cli/copy/v1\n            externalParameters:\n                source: https://example.com/upstream\n"
        );
        assert_eq!(adr_state(Some(&sidecar), content).0, "copied");
        assert_eq!(adr_state(Some(&sidecar), "edited body\n").0, "modified");
    }
}
