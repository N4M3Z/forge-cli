//! Pure dashboard view builders shared by the web UI and terminal UI.

use crate::view::{
    Adoption, ArtifactView, DashboardView, DeployGroup, DeployHarness, KIND_ORDER, ModuleView,
    ProvenanceArtifact,
};

/// A primary-facet section in the nested overview.
pub struct NestedGroup<'a> {
    pub label: String,
    pub kind: String,
    pub count: usize,
    pub subgroups: Vec<NestedSub<'a>>,
}

/// A secondary-facet sub-section holding artifact rows.
pub struct NestedSub<'a> {
    pub label: String,
    pub kind: String,
    pub count: usize,
    pub items: Vec<&'a ArtifactView>,
}

/// Count matrix: rows = modules, columns = kinds, cells = counts.
pub struct MatrixView {
    pub cols: Vec<String>,
    pub rows: Vec<MatrixRow>,
    pub col_totals: Vec<usize>,
    pub total: usize,
}

pub struct MatrixRow {
    pub module: String,
    pub cells: Vec<MatrixCell>,
    pub total: usize,
}

pub struct MatrixCell {
    pub kind: String,
    pub module: String,
    pub count: usize,
    pub status: String,
}

/// Coverage grid for model/harness variants.
pub struct VariantCoverage {
    pub cols: Vec<VariantCol>,
    pub rows: Vec<VariantCoverageRow>,
    pub col_totals: Vec<usize>,
}

pub struct VariantCol {
    pub qualifier: String,
    pub provider: String,
    pub label: String,
}

pub struct VariantCoverageRow {
    pub module: String,
    pub kind: String,
    pub name: String,
    pub cells: Vec<VariantCoverageCell>,
}

pub struct VariantCoverageCell {
    pub mode: String,
    pub link: String,
}

/// A resolved adoption dependency.
pub struct DepLink {
    pub name: String,
    pub uri: String,
    pub module: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchFilters {
    pub query: String,
    pub kind: String,
    pub module: String,
    pub status: String,
    pub sort: String,
}

impl SearchFilters {
    #[must_use]
    pub fn empty() -> Self {
        Self {
            query: String::new(),
            kind: String::new(),
            module: String::new(),
            status: String::new(),
            sort: String::new(),
        }
    }
}

/// Worst (most attention-worthy) status among a set of artifacts, for a cell dot.
fn worst_status(items: &[&ArtifactView]) -> String {
    let rank = |status: &str| match status {
        "modified" => 4,
        "stale" => 3,
        "new" => 2,
        "source" => 1,
        _ => 0,
    };
    items
        .iter()
        .map(|item| item.overall_status())
        .max_by_key(|status| rank(status))
        .unwrap_or("ok")
        .to_string()
}

/// Builds the nested two-facet grouping.
#[must_use]
pub fn build_nested<'a>(view: &'a DashboardView, primary: &str) -> Vec<NestedGroup<'a>> {
    if primary == "module" {
        view.modules
            .iter()
            .filter_map(|module| {
                let subgroups: Vec<NestedSub<'_>> = KIND_ORDER
                    .iter()
                    .filter_map(|&kind| kind_sub(module, kind))
                    .collect();
                build_group(module.name.clone(), String::new(), subgroups)
            })
            .collect()
    } else {
        KIND_ORDER
            .iter()
            .filter_map(|&kind| {
                let subgroups: Vec<NestedSub<'_>> = view
                    .modules
                    .iter()
                    .filter_map(|module| module_sub(module, kind))
                    .collect();
                build_group(kind.to_string(), kind.to_string(), subgroups)
            })
            .collect()
    }
}

fn items_of_kind<'a>(module: &'a ModuleView, kind: &str) -> Vec<&'a ArtifactView> {
    let mut items: Vec<&ArtifactView> = module
        .artifacts
        .iter()
        .filter(|artifact| artifact.kind == kind)
        .collect();
    items.sort_by(|a, b| {
        b.total_lines()
            .cmp(&a.total_lines())
            .then_with(|| a.name.cmp(&b.name))
    });
    items
}

fn kind_sub<'a>(module: &'a ModuleView, kind: &str) -> Option<NestedSub<'a>> {
    let items = items_of_kind(module, kind);
    (!items.is_empty()).then(|| NestedSub {
        label: kind.to_string(),
        kind: kind.to_string(),
        count: items.len(),
        items,
    })
}

fn module_sub<'a>(module: &'a ModuleView, kind: &str) -> Option<NestedSub<'a>> {
    let items = items_of_kind(module, kind);
    (!items.is_empty()).then(|| NestedSub {
        label: module.name.clone(),
        kind: String::new(),
        count: items.len(),
        items,
    })
}

fn build_group(
    label: String,
    kind: String,
    subgroups: Vec<NestedSub<'_>>,
) -> Option<NestedGroup<'_>> {
    if subgroups.is_empty() {
        return None;
    }
    let count = subgroups.iter().map(|sub| sub.count).sum();
    Some(NestedGroup {
        label,
        kind,
        count,
        subgroups,
    })
}

/// Builds the count matrix (modules x kinds) with row/column totals.
#[must_use]
pub fn build_matrix(view: &DashboardView) -> MatrixView {
    let cols: Vec<String> = KIND_ORDER.iter().map(|&kind| kind.to_string()).collect();
    let mut col_totals = vec![0usize; cols.len()];
    let mut total = 0usize;
    let rows = view
        .modules
        .iter()
        .map(|module| {
            let mut row_total = 0usize;
            let cells = KIND_ORDER
                .iter()
                .enumerate()
                .map(|(index, &kind)| {
                    let items: Vec<&ArtifactView> = module
                        .artifacts
                        .iter()
                        .filter(|artifact| artifact.kind == kind)
                        .collect();
                    let count = items.len();
                    row_total += count;
                    col_totals[index] += count;
                    total += count;
                    MatrixCell {
                        kind: kind.to_string(),
                        module: module.name.clone(),
                        count,
                        status: if count == 0 {
                            String::new()
                        } else {
                            worst_status(&items)
                        },
                    }
                })
                .collect();
            MatrixRow {
                module: module.name.clone(),
                cells,
                total: row_total,
            }
        })
        .collect();
    MatrixView {
        cols,
        rows,
        col_totals,
        total,
    }
}

/// Builds the variant-coverage grid across every artifact that has qualifier overrides.
#[must_use]
pub fn build_variant_coverage(view: &DashboardView) -> VariantCoverage {
    let mut qualifiers: Vec<String> = Vec::new();
    for module in &view.modules {
        for artifact in &module.artifacts {
            for variant in &artifact.variants {
                if !qualifiers.contains(&variant.qualifier) {
                    qualifiers.push(variant.qualifier.clone());
                }
            }
        }
    }
    qualifiers.sort();
    let cols: Vec<VariantCol> = qualifiers
        .iter()
        .map(|qualifier| {
            let (provider, label) = qualifier.split_once('/').map_or(
                (qualifier.as_str(), qualifier.as_str()),
                |(provider, model)| (provider, model),
            );
            VariantCol {
                qualifier: qualifier.clone(),
                provider: provider.to_string(),
                label: label.to_string(),
            }
        })
        .collect();

    let mut col_totals = vec![0usize; cols.len()];
    let mut rows = Vec::new();
    for module in &view.modules {
        for artifact in &module.artifacts {
            if artifact.variants.is_empty() {
                continue;
            }
            let cells = cols
                .iter()
                .enumerate()
                .map(|(index, col)| {
                    match artifact
                        .variants
                        .iter()
                        .find(|variant| variant.qualifier == col.qualifier)
                    {
                        Some(variant) => {
                            col_totals[index] += 1;
                            VariantCoverageCell {
                                mode: variant.mode.clone(),
                                link: format!(
                                    "/effective/{}/{}/{}?qualifier={}",
                                    module.name, artifact.kind, artifact.name, col.qualifier
                                ),
                            }
                        }
                        None => VariantCoverageCell {
                            mode: String::new(),
                            link: String::new(),
                        },
                    }
                })
                .collect();
            rows.push(VariantCoverageRow {
                module: module.name.clone(),
                kind: artifact.kind.clone(),
                name: artifact.name.clone(),
                cells,
            });
        }
    }
    VariantCoverage {
        cols,
        rows,
        col_totals,
    }
}

/// Resolves each adoption dependency to the module containing a skill of that name.
#[must_use]
pub fn resolve_dep_links(view: &DashboardView, adoption: Option<&Adoption>) -> Vec<DepLink> {
    let Some(adoption) = adoption else {
        return Vec::new();
    };
    adoption
        .dependencies
        .iter()
        .map(|dependency| {
            let module = view
                .modules
                .iter()
                .find(|candidate| {
                    candidate.artifacts.iter().any(|artifact| {
                        artifact.kind == "skills" && artifact.name == dependency.name
                    })
                })
                .map_or_else(String::new, |candidate| candidate.name.clone());
            DepLink {
                name: dependency.name.clone(),
                uri: dependency.uri.clone(),
                module,
            }
        })
        .collect()
}

/// Groups deployment provenance entries by target location.
#[must_use]
pub fn group_deployments(entries: &[&ProvenanceArtifact]) -> Vec<DeployGroup> {
    let mut order: Vec<String> = Vec::new();
    let mut groups: std::collections::HashMap<String, DeployGroup> =
        std::collections::HashMap::new();
    for entry in entries {
        let group = groups.entry(entry.target.clone()).or_insert_with(|| {
            order.push(entry.target.clone());
            DeployGroup {
                target: entry.target.clone(),
                verified: 0,
                total: 0,
                harnesses: Vec::new(),
            }
        });
        group.total += 1;
        if entry.verified {
            group.verified += 1;
        }
        let deployed_dir = entry
            .deployed_path
            .rsplit_once('/')
            .map_or_else(String::new, |(dir, _)| dir.to_string());
        group.harnesses.push(DeployHarness {
            harness: entry.harness.clone(),
            deployed_path: entry.deployed_path.clone(),
            deployed_dir,
            verified: entry.verified,
        });
    }
    for group in groups.values_mut() {
        group.harnesses.sort_by(|a, b| a.harness.cmp(&b.harness));
    }
    order
        .into_iter()
        .filter_map(|target| groups.remove(&target))
        .collect()
}

/// Whether an artifact matches a status filter value.
#[must_use]
pub fn matches_status(artifact: &ArtifactView, status: &str) -> bool {
    if status == "attention" {
        return artifact.has_broken_refs()
            || matches!(artifact.overall_status(), "modified" | "stale");
    }
    artifact.overall_status() == status
}

/// Sorts matched artifacts in place.
pub fn sort_results(matched: &mut [(&ArtifactView, &str)], sort: &str) {
    match sort {
        "name" => matched.sort_by(|a, b| a.0.name.cmp(&b.0.name)),
        "module" => matched.sort_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.name.cmp(&b.0.name))),
        "size" => matched.sort_by(|a, b| {
            b.0.total_lines()
                .cmp(&a.0.total_lines())
                .then_with(|| a.0.name.cmp(&b.0.name))
        }),
        "age" => matched.sort_by(|a, b| match (a.0.age_days, b.0.age_days) {
            (Some(left), Some(right)) => right.cmp(&left).then_with(|| a.0.name.cmp(&b.0.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.0.name.cmp(&b.0.name),
        }),
        "staleness" => matched.sort_by(|a, b| {
            a.0.staleness_rank()
                .cmp(&b.0.staleness_rank())
                .reverse()
                .then_with(|| a.0.name.cmp(&b.0.name))
        }),
        _ => matched.sort_by(|a, b| {
            let a_date = a.0.latest_commit_date();
            let b_date = b.0.latest_commit_date();
            match (a_date.is_empty(), b_date.is_empty()) {
                (true, true) => a.0.name.cmp(&b.0.name),
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                (false, false) => b_date.cmp(a_date).then_with(|| a.0.name.cmp(&b.0.name)),
            }
        }),
    }
}

/// Returns filtered and sorted artifact refs for search surfaces.
#[must_use]
pub fn search_results<'a>(
    view: &'a DashboardView,
    filters: &SearchFilters,
) -> Vec<(&'a ArtifactView, &'a str)> {
    let query_lower = filters.query.to_lowercase();
    let mut matched: Vec<(&ArtifactView, &str)> = view
        .modules
        .iter()
        .filter(|module| filters.module.is_empty() || module.name == filters.module)
        .flat_map(|module| {
            module
                .artifacts
                .iter()
                .map(move |artifact| (artifact, module.name.as_str()))
        })
        .filter(|(artifact, _)| {
            if !filters.kind.is_empty() && artifact.kind != filters.kind {
                return false;
            }
            if !filters.status.is_empty() && !matches_status(artifact, &filters.status) {
                return false;
            }
            query_lower.is_empty() || artifact.matches_query(&query_lower)
        })
        .collect();
    sort_results(&mut matched, &filters.sort);
    matched
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::{StatusSummary, Variant};

    fn artifact(kind: &str, name: &str, lines: &str) -> ArtifactView {
        ArtifactView {
            name: name.to_string(),
            kind: kind.to_string(),
            relative_path: format!("{kind}/{name}.md"),
            raw_source: lines.to_string(),
            ..Default::default()
        }
    }

    fn variant(qualifier: &str, provider: &str, model: &str, mode: &str) -> Variant {
        Variant {
            qualifier: qualifier.to_string(),
            provider: provider.to_string(),
            model: model.to_string(),
            relative_path: format!("rules/{provider}/{model}/Rule.md"),
            mode: mode.to_string(),
        }
    }

    fn rule_with_variants(name: &str, variants: Vec<Variant>) -> ArtifactView {
        ArtifactView {
            name: name.to_string(),
            kind: "rules".to_string(),
            relative_path: format!("rules/{name}.md"),
            variants,
            ..Default::default()
        }
    }

    fn module(name: &str, artifacts: Vec<ArtifactView>) -> ModuleView {
        ModuleView {
            name: name.to_string(),
            version: String::new(),
            description: String::new(),
            source_uri: format!("https://example.com/{name}"),
            is_target: false,
            artifacts,
        }
    }

    fn view(modules: Vec<ModuleView>) -> DashboardView {
        DashboardView {
            modules,
            summary: StatusSummary::default(),
            provenance: Vec::new(),
            adrs: Vec::new(),
        }
    }

    #[test]
    fn nested_groups_by_kind_with_largest_items_first() {
        let overview = view(vec![module(
            "forge-core",
            vec![
                artifact("skills", "Small", "one"),
                artifact("skills", "Large", "one\ntwo\nthree"),
                artifact("rules", "Rule", "one"),
            ],
        )]);

        let nested = build_nested(&overview, "kind");

        assert_eq!(nested[0].label, "skills");
        assert_eq!(nested[0].count, 2);
        assert_eq!(nested[0].subgroups[0].label, "forge-core");
        assert_eq!(nested[0].subgroups[0].items[0].name, "Large");
        assert_eq!(nested[1].label, "rules");
    }

    #[test]
    fn matrix_counts_modules_by_kind() {
        let matrix = build_matrix(&view(vec![module(
            "forge-core",
            vec![
                artifact("skills", "BuildSkill", "one"),
                artifact("rules", "Rule", "one"),
            ],
        )]));

        assert_eq!(matrix.cols, vec!["skills", "agents", "rules"]);
        assert_eq!(matrix.rows[0].module, "forge-core");
        assert_eq!(matrix.rows[0].cells[0].count, 1);
        assert_eq!(matrix.rows[0].cells[1].count, 0);
        assert_eq!(matrix.rows[0].cells[2].count, 1);
        assert_eq!(matrix.total, 2);
    }

    #[test]
    fn coverage_excludes_artifacts_without_variants() {
        let covered = rule_with_variants(
            "DeadVariables",
            vec![variant("claude", "claude", "", "replace")],
        );
        let bare = rule_with_variants("PlainRule", Vec::new());
        let coverage =
            build_variant_coverage(&view(vec![module("forge-core", vec![bare, covered])]));
        assert_eq!(coverage.rows.len(), 1);
        assert_eq!(coverage.rows[0].name, "DeadVariables");
    }

    #[test]
    fn coverage_columns_sorted_and_split_into_provider_and_model() {
        let artifact = rule_with_variants(
            "DeadVariables",
            vec![
                variant(
                    "claude/claude-opus-4-8",
                    "claude",
                    "claude-opus-4-8",
                    "append",
                ),
                variant("claude", "claude", "", "replace"),
            ],
        );
        let coverage = build_variant_coverage(&view(vec![module("forge-core", vec![artifact])]));

        let labels: Vec<&str> = coverage.cols.iter().map(|col| col.label.as_str()).collect();
        assert_eq!(labels, vec!["claude", "claude-opus-4-8"]);
        assert_eq!(coverage.cols[1].provider, "claude");
        assert_eq!(coverage.cols[1].qualifier, "claude/claude-opus-4-8");
    }

    #[test]
    fn coverage_cells_carry_mode_link_and_totals() {
        let artifact = rule_with_variants(
            "DeadVariables",
            vec![
                variant("claude", "claude", "", "replace"),
                variant(
                    "claude/claude-opus-4-8",
                    "claude",
                    "claude-opus-4-8",
                    "append",
                ),
            ],
        );
        let coverage = build_variant_coverage(&view(vec![module("forge-core", vec![artifact])]));
        let row = &coverage.rows[0];

        assert_eq!(row.cells[0].mode, "replace");
        assert_eq!(
            row.cells[0].link,
            "/effective/forge-core/rules/DeadVariables?qualifier=claude"
        );
        assert_eq!(row.cells[1].mode, "append");
        assert_eq!(
            row.cells[1].link,
            "/effective/forge-core/rules/DeadVariables?qualifier=claude/claude-opus-4-8"
        );
        assert_eq!(coverage.col_totals, vec![1, 1]);
    }

    #[test]
    fn coverage_empty_cell_when_target_missing() {
        let with_model = rule_with_variants(
            "HasModel",
            vec![variant(
                "claude/claude-opus-4-8",
                "claude",
                "claude-opus-4-8",
                "replace",
            )],
        );
        let provider_only = rule_with_variants(
            "ProviderOnly",
            vec![variant("claude", "claude", "", "replace")],
        );
        let coverage = build_variant_coverage(&view(vec![module(
            "forge-core",
            vec![with_model, provider_only],
        )]));

        let has_model = coverage
            .rows
            .iter()
            .find(|row| row.name == "HasModel")
            .expect("variant row");
        assert!(has_model.cells[0].mode.is_empty());
        assert!(has_model.cells[0].link.is_empty());
        assert_eq!(has_model.cells[1].mode, "replace");
    }
}
