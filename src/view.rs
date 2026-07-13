//! Shared view-model types consumed by the web dashboard (and a future TUI).
//!
//! These live in the lib crate so a later rendering surface can share the same
//! data shapes. Today only `cli::dashboard::scan` populates them, from existing
//! lib functions.

use crate::manifest::FileStatus;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
pub struct DashboardView {
    pub modules: Vec<ModuleView>,
    pub summary: StatusSummary,
    pub provenance: Vec<ProvenanceView>,
    pub adrs: Vec<Adr>,
}

/// An architecture decision record found in a repo's `docs/decisions/`.
#[derive(Debug, Clone, Serialize)]
pub struct Adr {
    pub id: String,
    pub title: String,
    pub status: String,
    pub repo: String,
    pub source_uri: String,
    pub relative_path: String,
    /// Lifecycle state: `authored`, `copied`, or `modified`.
    pub state: String,
    /// Where an adopted ADR was copied from (empty when authored locally).
    pub source: String,
    /// First prose paragraph (Context section when present), for the list preview.
    pub summary: String,
    /// Absolute path of the ADR file on disk, for full-document rendering.
    pub local_path: String,
}

/// One repo's ADRs in the list view: the repo label, its total, and the ADRs
/// sub-grouped by id prefix.
pub struct AdrRepoGroup<'a> {
    pub repo: &'a str,
    pub total: usize,
    pub prefix_groups: Vec<AdrPrefixGroup<'a>>,
}

/// A consecutive run of ADRs sharing an id prefix (e.g. all `ASSEMBLY-*`).
pub struct AdrPrefixGroup<'a> {
    pub prefix: String,
    pub adrs: Vec<&'a Adr>,
}

impl DashboardView {
    /// ADRs grouped by repo (first-seen order), then sub-grouped by id prefix
    /// within each repo.
    #[must_use]
    pub fn adrs_grouped(&self) -> Vec<AdrRepoGroup<'_>> {
        let mut order: Vec<&str> = Vec::new();
        let mut groups: std::collections::HashMap<&str, Vec<&Adr>> =
            std::collections::HashMap::new();
        for adr in &self.adrs {
            if !groups.contains_key(adr.repo.as_str()) {
                order.push(adr.repo.as_str());
            }
            groups.entry(adr.repo.as_str()).or_default().push(adr);
        }
        order
            .into_iter()
            .map(|repo| {
                let adrs = groups.remove(repo).unwrap_or_default();
                AdrRepoGroup {
                    repo,
                    total: adrs.len(),
                    prefix_groups: group_by_prefix(adrs),
                }
            })
            .collect()
    }
}

/// Splits an ADR id into its prefix: `ASSEMBLY-0001` -> `ASSEMBLY`, `ARCH-0003`
/// -> `ARCH`. Falls back to the whole id when there is no `-<digits>` suffix.
fn adr_prefix(id: &str) -> String {
    match id.rsplit_once('-') {
        Some((prefix, suffix))
            if !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit()) =>
        {
            prefix.to_string()
        }
        _ => id.to_string(),
    }
}

/// Groups already-ordered ADRs into consecutive runs sharing an id prefix.
fn group_by_prefix(adrs: Vec<&Adr>) -> Vec<AdrPrefixGroup<'_>> {
    let mut groups: Vec<AdrPrefixGroup<'_>> = Vec::new();
    for adr in adrs {
        let prefix = adr_prefix(&adr.id);
        match groups.last_mut() {
            Some(group) if group.prefix == prefix => group.adrs.push(adr),
            _ => groups.push(AdrPrefixGroup {
                prefix,
                adrs: vec![adr],
            }),
        }
    }
    groups
}

impl DashboardView {
    /// Canon repos (discovered through their deployed artifacts), in list order.
    #[must_use]
    pub fn source_modules(&self) -> Vec<&ModuleView> {
        self.modules
            .iter()
            .filter(|module| !module.is_target)
            .collect()
    }

    /// Repos added via `forge watch` (downstream targets), in list order.
    #[must_use]
    pub fn target_modules(&self) -> Vec<&ModuleView> {
        self.modules
            .iter()
            .filter(|module| module.is_target)
            .collect()
    }

    pub fn all_artifacts(&self) -> Vec<(&ArtifactView, &str)> {
        self.modules
            .iter()
            .flat_map(|module| {
                module
                    .artifacts
                    .iter()
                    .map(move |artifact| (artifact, module.name.as_str()))
            })
            .collect()
    }

    /// Artifacts grouped by kind in display order (skills, agents, rules),
    /// each sorted by most recent commit. Empty kinds are omitted.
    pub fn artifacts_by_kind(&self) -> Vec<(&'static str, Vec<(&ArtifactView, &str)>)> {
        let mut grouped = Vec::new();
        for kind in KIND_ORDER {
            let mut items: Vec<(&ArtifactView, &str)> = self
                .modules
                .iter()
                .flat_map(|module| {
                    module
                        .artifacts
                        .iter()
                        .map(move |artifact| (artifact, module.name.as_str()))
                })
                .filter(|(artifact, _)| artifact.kind == kind)
                .collect();
            if items.is_empty() {
                continue;
            }
            items.sort_by(|a, b| compare_recent(a.0, b.0));
            grouped.push((kind, items));
        }
        grouped
    }
}

/// Display order for content kinds: skills first.
pub const KIND_ORDER: [&str; 3] = ["skills", "agents", "rules"];

/// Groups artifact references by kind in display order (skills, agents, rules),
/// preserving the incoming order within each kind. Empty kinds are omitted.
#[must_use]
pub fn group_by_kind<'a>(
    artifacts: &[&'a ArtifactView],
) -> Vec<(&'static str, Vec<&'a ArtifactView>)> {
    KIND_ORDER
        .into_iter()
        .filter_map(|kind| {
            let items: Vec<&ArtifactView> = artifacts
                .iter()
                .filter(|artifact| artifact.kind == kind)
                .copied()
                .collect();
            (!items.is_empty()).then_some((kind, items))
        })
        .collect()
}

/// Recent-commit ordering: newest first, artifacts with no git history last,
/// ties broken by name.
fn compare_recent(a: &ArtifactView, b: &ArtifactView) -> std::cmp::Ordering {
    let a_date = a.latest_commit_date();
    let b_date = b.latest_commit_date();
    match (a_date.is_empty(), b_date.is_empty()) {
        (true, true) => a.name.cmp(&b.name),
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => b_date.cmp(a_date).then_with(|| a.name.cmp(&b.name)),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleView {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source_uri: String,
    /// True for repos added via `forge watch` (downstream targets); false for
    /// the canon sources discovered through their deployed artifacts.
    pub is_target: bool,
    pub artifacts: Vec<ArtifactView>,
    /// Local clone of the module's source repo, when one was discovered.
    pub local_path: Option<std::path::PathBuf>,
    /// Repo-level version-control state (branch, ahead/behind, dirty).
    pub vcs: Option<VcsState>,
    /// Most recent commits across the whole repo.
    pub git_log: Vec<GitCommit>,
}

impl ModuleView {
    /// This module's artifacts grouped by kind in display order (skills, agents,
    /// rules), preserving the existing per-kind sort. Empty kinds are omitted.
    #[must_use]
    pub fn artifacts_by_kind(&self) -> Vec<(&'static str, Vec<&ArtifactView>)> {
        let refs: Vec<&ArtifactView> = self.artifacts.iter().collect();
        group_by_kind(&refs)
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ArtifactView {
    pub name: String,
    pub kind: String,
    pub module: String,
    pub relative_path: String,
    /// Path of the source file relative to the module repo, when known. Falls
    /// back to `relative_path` (the deploy key) for VCS matching — the two
    /// diverge once modules live inside a monorepo.
    pub source_path: String,
    pub description: String,
    pub content_preview: String,
    pub content_body: String,
    pub raw_source: String,
    pub metadata: Vec<(String, String)>,
    pub providers: BTreeMap<String, ProviderStatus>,
    pub git_log: Vec<GitCommit>,
    pub adoption: Option<Adoption>,
    pub sidecar_warning: String,
    /// Intra-repo markdown link targets cited by this artifact that no longer
    /// resolve on disk (the reference-integrity staleness signal).
    pub broken_refs: Vec<String>,
    /// Days since the most recent commit touching this artifact (a faint age
    /// hint, never a staleness verdict on its own).
    pub age_days: Option<i64>,
    /// Palette slot (0-7) for the faint per-module card tint, assigned by the
    /// module's position so co-located modules stay visually distinct.
    pub module_tint: usize,
    pub companions: Vec<Companion>,
    /// Per-harness and per-model qualifier overrides found in the source tree
    /// (the model-targeting variants from PROV-0005), empty when none.
    pub variants: Vec<Variant>,
    /// Version-control state of the artifact's source file, `None` when the
    /// module has no local repo.
    pub vcs: Option<VcsState>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct VcsState {
    pub branch: String,
    pub worktree: WorktreeState,
    /// Commits on HEAD not yet on the upstream, and vice versa. Both zero when
    /// the branch has no upstream.
    pub ahead: usize,
    pub behind: usize,
    pub jj_colocated: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub enum WorktreeState {
    Clean,
    Modified,
    Untracked,
}

/// A harness- or model-qualifier override of a base artifact, discovered in the
/// source tree (e.g. `rules/claude/claude-opus-4-8/DeadVariables.md`).
#[derive(Debug, Clone, Serialize)]
pub struct Variant {
    /// Qualifier path under the kind dir: `claude`, `claude/claude-opus-4-8`, `user`.
    pub qualifier: String,
    /// The harness this qualifier belongs to (`claude`, `gemini`, `user`).
    pub provider: String,
    /// The model directory when this is a model-level variant, else empty.
    pub model: String,
    /// Repo-relative path to the variant file.
    pub relative_path: String,
    /// Merge mode from the variant frontmatter: `replace` (default), `append`, `prepend`.
    pub mode: String,
}

impl ArtifactView {
    #[must_use]
    pub fn has_variants(&self) -> bool {
        !self.variants.is_empty()
    }

    /// Distinct harnesses that carry a variant of this artifact, in stable order.
    #[must_use]
    pub fn variant_providers(&self) -> Vec<String> {
        let mut seen = Vec::new();
        for variant in &self.variants {
            if !seen.contains(&variant.provider) {
                seen.push(variant.provider.clone());
            }
        }
        seen
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Companion {
    pub name: String,
    pub relative_path: String,
    pub description: String,
    pub content_body: String,
    pub raw_source: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Adoption {
    pub kind: String,
    pub source: String,
    pub source_repo: String,
    pub source_label: String,
    pub source_sha: String,
    pub commit: String,
    pub transforms: Vec<String>,
    pub author: String,
    pub license: String,
    pub adopted_by: String,
    pub dependencies: Vec<Dependency>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Dependency {
    pub name: String,
    pub uri: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct GitCommit {
    pub sha: String,
    pub message: String,
    pub date: String,
    pub author: String,
    /// `Entire-Checkpoint` trailer (12-hex) linking this commit to an agent
    /// checkpoint, empty when the commit predates Entire or carries no trailer.
    pub checkpoint: String,
    /// First non-continuation agent prompt in the checkpoint, a one-line teaser
    /// of the intent behind this commit. Empty when there is no checkpoint.
    pub prompt: String,
    /// Number of agent sessions condensed into this commit's checkpoint.
    pub session_count: usize,
    /// Jujutsu change-id (short) for this commit in a colocated repo, empty when
    /// the repo is not jj-colocated or the commit has no jj change.
    pub jj_change: String,
}

impl ArtifactView {
    pub fn status_for(&self, provider: &str) -> Option<&ProviderStatus> {
        self.providers.get(provider)
    }

    /// Matches a lowercased query against the artifact name, description, and
    /// any companion as `SkillName/CompanionName`.
    pub fn matches_query(&self, query_lower: &str) -> bool {
        if self.name.to_lowercase().contains(query_lower)
            || self.description.to_lowercase().contains(query_lower)
        {
            return true;
        }
        self.companions.iter().any(|companion| {
            let qualified = format!("{}/{}", self.name, companion.name).to_lowercase();
            qualified.contains(query_lower) || companion.name.to_lowercase().contains(query_lower)
        })
    }

    pub fn overall_status(&self) -> &'static str {
        if self.providers.is_empty() {
            return "source";
        }
        if self
            .providers
            .values()
            .any(|provider| provider.status == FileStatus::Modified)
        {
            return "modified";
        }
        if self
            .providers
            .values()
            .any(|provider| provider.status == FileStatus::Stale)
        {
            return "stale";
        }
        if self
            .providers
            .values()
            .all(|provider| provider.status == FileStatus::New)
        {
            return "new";
        }
        "ok"
    }

    pub fn deployed_count(&self) -> usize {
        self.providers.len()
    }

    /// Per-harness status breakdown for the aggregate pill tooltip,
    /// e.g. `claude: unchanged, codex: stale`.
    pub fn provider_breakdown(&self) -> String {
        self.providers
            .iter()
            .map(|(provider, status)| format!("{provider}: {}", status.status.label()))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Latest git commit date (`%ai` ISO form) for `recent` sort, or empty
    /// when the artifact has no git history (sorts last).
    pub fn latest_commit_date(&self) -> &str {
        self.git_log
            .first()
            .map_or("", |commit| commit.date.as_str())
    }

    pub fn id(&self) -> String {
        format!("{}-{}", self.kind, self.name)
    }

    pub fn has_broken_refs(&self) -> bool {
        !self.broken_refs.is_empty()
    }

    /// Total source lines including companion files, used to rank the largest
    /// artifacts first in the overview peek.
    pub fn total_lines(&self) -> usize {
        self.raw_source.lines().count()
            + self
                .companions
                .iter()
                .map(|companion| companion.raw_source.lines().count())
                .sum::<usize>()
    }

    /// Compact relative age of the last commit, e.g. `today`, `12d`, `3mo`,
    /// `2y`. Empty when the artifact has no git history.
    pub fn age_label(&self) -> String {
        match self.age_days {
            None => String::new(),
            Some(days) if days < 1 => "today".to_string(),
            Some(days) if days < 30 => format!("{days}d"),
            Some(days) if days < 365 => format!("{}mo", days / 30),
            Some(days) => format!("{}y", days / 365),
        }
    }

    /// Composite staleness rank (higher = more stale), the single source of
    /// truth for the "staleness" sort and the artifact-view indicator. Broken
    /// references dominate (scaled by count), then deploy drift: modified over
    /// stale. A clean artifact ranks zero.
    pub fn staleness_rank(&self) -> usize {
        let mut rank = 0;
        if self.has_broken_refs() {
            rank += 100 + self.broken_refs.len();
        }
        rank += match self.overall_status() {
            "modified" => 50,
            "stale" => 30,
            _ => 0,
        };
        rank
    }

    /// CSS-friendly level for the worst staleness signal.
    pub fn staleness_level(&self) -> &'static str {
        if self.has_broken_refs() {
            "broken"
        } else {
            match self.overall_status() {
                "modified" => "modified",
                "stale" => "stale",
                _ => "ok",
            }
        }
    }

    /// Human label for the composite staleness state, shown on the artifact view.
    pub fn staleness_label(&self) -> String {
        if self.has_broken_refs() {
            let count = self.broken_refs.len();
            return format!(
                "{count} broken reference{}",
                if count == 1 { "" } else { "s" }
            );
        }
        match self.overall_status() {
            "modified" => "deployed file modified".to_string(),
            "stale" => "source moved since deploy".to_string(),
            _ => "fresh".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderStatus {
    pub status: FileStatus,
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceView {
    pub source_uri: String,
    pub verified: usize,
    pub total: usize,
    pub orphans: Vec<String>,
    pub artifacts: Vec<ProvenanceArtifact>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProvenanceArtifact {
    pub deployed_path: String,
    pub source_path: String,
    pub harness: String,
    pub target: String,
    pub verified: bool,
    pub deployed_sha: String,
    pub expected_sha: String,
    pub input_sha: String,
}

/// Deployment entries for one artifact grouped by target location, so the
/// provenance graph shows one node per directory (expandable to harnesses)
/// instead of an unbounded flat list.
#[derive(Debug, Clone, Serialize)]
pub struct DeployGroup {
    pub target: String,
    pub verified: usize,
    pub total: usize,
    pub harnesses: Vec<DeployHarness>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeployHarness {
    pub harness: String,
    pub deployed_path: String,
    pub deployed_dir: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct StatusSummary {
    pub unchanged: usize,
    pub stale: usize,
    pub modified: usize,
    pub new: usize,
}
