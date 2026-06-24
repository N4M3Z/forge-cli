use commands::view::{ArtifactView, DashboardView, KIND_ORDER, ModuleView};

use super::views::{
    MatrixCell, MatrixRow, MatrixView, NestedGroup, NestedSub, VariantCol, VariantCoverage,
    VariantCoverageCell, VariantCoverageRow,
};

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

/// Builds the nested two-facet grouping. `primary` is the outer facet
/// (`kind` or `module`); the other facet becomes the inner sub-groups. Only
/// non-empty groups are emitted, so ragged data produces no empty sections.
pub fn build_nested<'a>(view: &'a DashboardView, primary: &str) -> Vec<NestedGroup<'a>> {
    if primary == "module" {
        view.modules
            .iter()
            .filter_map(|module| {
                let subgroups: Vec<NestedSub> = KIND_ORDER
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
                let subgroups: Vec<NestedSub> = view
                    .modules
                    .iter()
                    .filter_map(|module| module_sub(module, kind))
                    .collect();
                build_group(kind.to_string(), kind.to_string(), subgroups)
            })
            .collect()
    }
}

/// A module's artifacts of one kind, largest first (total lines incl. companion
/// files), so the overview peek surfaces the most substantial artifacts.
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

fn build_group(label: String, kind: String, subgroups: Vec<NestedSub>) -> Option<NestedGroup> {
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

/// Builds the count matrix (modules × kinds) with row/column totals.
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

/// Builds the variant-coverage grid across every artifact that has qualifier
/// overrides. Columns are sorted so provider-level qualifiers precede their
/// per-model children (lexical order places `claude` before `claude/...`).
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
            let (provider, label) = qualifier
                .split_once('/')
                .map_or((qualifier.as_str(), qualifier.as_str()), |(p, m)| (p, m));
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

#[cfg(test)]
mod tests {
    use super::*;
    use commands::view::{StatusSummary, Variant};

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

        // Columns: ["claude", "claude/claude-opus-4-8"]; HasModel has no "claude" cell.
        let has_model = coverage
            .rows
            .iter()
            .find(|row| row.name == "HasModel")
            .unwrap();
        assert!(has_model.cells[0].mode.is_empty());
        assert!(has_model.cells[0].link.is_empty());
        assert_eq!(has_model.cells[1].mode, "replace");
    }
}
