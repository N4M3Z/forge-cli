use askama::Template;
use commands::view::{ArtifactView, DashboardView, KIND_ORDER, ModuleView};

/// Shared status + search bar loaded into card-list views via htmx.
#[derive(Template)]
#[template(path = "dashboard/chrome.html")]
pub struct ChromeTemplate<'a> {
    pub view: &'a DashboardView,
    pub scanned_at: &'a str,
}

#[derive(Template)]
#[template(path = "dashboard/overview.html")]
pub struct OverviewTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub view: &'a DashboardView,
    pub scanned_at: &'a str,
    pub layout: &'a str,
    pub primary: &'a str,
    pub density: &'a str,
    pub nested: Vec<NestedGroup<'a>>,
    pub matrix: Option<MatrixView>,
}

/// A primary-facet section in the nested overview (e.g. one kind, or one module).
pub struct NestedGroup<'a> {
    pub label: String,
    /// Kind name when the outer facet is kind (drives the color class); else empty.
    pub kind: String,
    pub count: usize,
    pub subgroups: Vec<NestedSub<'a>>,
}

/// A secondary-facet sub-section holding the artifact rows.
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
    /// Worst status among the cell's artifacts, or empty when the cell is empty.
    pub status: String,
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

/// Coverage grid for model/harness variants: rows = artifacts that carry at
/// least one qualifier override, columns = the distinct qualifiers seen across
/// all such artifacts, cells = the merge mode for that (artifact, qualifier).
#[derive(Template)]
#[template(path = "dashboard/variants.html")]
pub struct VariantsTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub coverage: VariantCoverage,
}

pub struct VariantCoverage {
    pub cols: Vec<VariantCol>,
    pub rows: Vec<VariantCoverageRow>,
    pub col_totals: Vec<usize>,
}

pub struct VariantCol {
    pub qualifier: String,
    /// Provider segment, drives the column tint.
    pub provider: String,
    /// Short header label: the model segment, or the provider when there is none.
    pub label: String,
}

pub struct VariantCoverageRow {
    pub module: String,
    pub kind: String,
    pub name: String,
    pub cells: Vec<VariantCoverageCell>,
}

pub struct VariantCoverageCell {
    /// Merge mode (`replace`/`append`/`prepend`) when a variant exists, else empty.
    pub mode: String,
    /// `/effective/...` link for a present cell, else empty.
    pub link: String,
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

#[derive(Template)]
#[template(path = "dashboard/adrs.html")]
pub struct AdrsTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub view: &'a DashboardView,
}

#[derive(Template)]
#[template(path = "dashboard/modules.html")]
pub struct ModulesTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub view: &'a DashboardView,
    pub selected_module: &'a str,
    pub detail: Option<&'a ModuleView>,
}

#[derive(Template)]
#[template(path = "dashboard/module_detail.html")]
pub struct ModuleDetailTemplate<'a> {
    pub module: &'a ModuleView,
}

/// A resolved adoption dependency: its module is filled in when a scanned
/// module contains a skill of that name, so the chip can link to the right
/// copy. An empty module renders as plain text (no link).
pub struct DepLink {
    pub name: String,
    pub uri: String,
    pub module: String,
}

#[derive(Template)]
#[template(path = "dashboard/artifact_detail.html")]
pub struct ArtifactDetailTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub binary_hash: &'a str,
    pub artifact: &'a ArtifactView,
    pub module_name: &'a str,
    pub module_source_uri: &'a str,
    pub deploy_groups: Vec<commands::view::DeployGroup>,
    pub provenance_raw: String,
    pub diff_deployed: String,
    pub diff_source_at_deploy: String,
    pub dep_links: Vec<DepLink>,
    pub schema_applies: String,
}

#[derive(Template)]
#[template(path = "dashboard/search_results.html")]
pub struct SearchResultsTemplate<'a> {
    pub groups: Vec<(&'static str, Vec<&'a ArtifactView>)>,
    pub page: usize,
    pub total_pages: usize,
    pub total: usize,
    pub query: &'a str,
    pub kind: &'a str,
    pub module: &'a str,
    pub status: &'a str,
    pub sort: &'a str,
}

#[derive(Template)]
#[template(path = "dashboard/search_page.html")]
pub struct SearchPageTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub view: &'a DashboardView,
    pub scanned_at: &'a str,
    pub groups: Vec<(&'static str, Vec<&'a ArtifactView>)>,
    pub query: &'a str,
    pub kind: &'a str,
    pub page: usize,
    pub total_pages: usize,
    pub total: usize,
    pub module: &'a str,
    pub status: &'a str,
    pub sort: &'a str,
}

pub struct IntegrityProblem {
    pub kind: String,
    pub name: String,
    pub module: String,
    pub issue: String,
    pub detail: String,
}

#[derive(Template)]
#[template(path = "dashboard/provenance.html")]
pub struct ProvenanceTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub verified: usize,
    pub total: usize,
    pub stale: usize,
    pub modified: usize,
    pub drift: usize,
    pub orphans: usize,
    pub broken: usize,
    pub problems: Vec<IntegrityProblem>,
}

#[derive(Template)]
#[template(path = "dashboard/deployed.html")]
pub struct DeployedTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub harness: &'a str,
    pub path: &'a str,
    pub exists: bool,
    pub content_body: String,
    pub raw_source: String,
    pub source: String,
}

/// A read-only config/settings file shown in the dashboard.
pub struct ConfigFile {
    pub label: String,
    pub path: String,
    pub language: String,
    pub content: String,
}

/// One registered hook parsed from a settings.json `hooks` block.
pub struct HookEntry {
    pub event: String,
    pub matcher: String,
    pub command: String,
    pub source: String,
}

/// Settings/config files found in one harness's target directory.
pub struct HarnessFiles {
    pub harness: String,
    pub files: Vec<ConfigFile>,
}

/// Hooks parsed from one harness's settings files.
pub struct HarnessHooks {
    pub harness: String,
    pub hooks: Vec<HookEntry>,
}

/// `.mdschema` and `.manifest` files from one source (a repo or a deploy target).
pub struct SchemaGroup {
    pub source: String,
    pub files: Vec<ConfigFile>,
}

#[derive(Template)]
#[template(path = "dashboard/files.html")]
pub struct FilesTemplate<'a> {
    pub tab: &'a str,
    pub title: &'a str,
    pub blurb: &'a str,
    pub version: &'a str,
    pub files: Vec<ConfigFile>,
}

/// A file rendered as a card in a grid view; clicking opens its detail page.
pub struct FileCard {
    pub label: String,
    pub path: String,
    pub language: String,
    pub lines: usize,
    pub href: String,
}

/// A titled group of file cards (one harness, one repo, or untitled for a flat list).
pub struct FileCardGroup {
    pub title: String,
    pub cards: Vec<FileCard>,
}

#[derive(Template)]
#[template(path = "dashboard/file_grid.html")]
pub struct FileGridTemplate<'a> {
    pub tab: &'a str,
    pub title: &'a str,
    pub blurb: &'a str,
    pub version: &'a str,
    pub groups: Vec<FileCardGroup>,
}

/// One hook shown as a detail page (full command, source file).
#[derive(Template)]
#[template(path = "dashboard/hook_detail.html")]
pub struct HookDetailTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub harness: String,
    pub event: String,
    pub matcher: String,
    pub source: String,
    /// Shell wrapper stripped for highlighting (e.g. `sh -c`), empty if none.
    pub wrapper: String,
    pub command: String,
}

#[derive(Template)]
#[template(path = "dashboard/hooks.html")]
pub struct HooksTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub groups: Vec<HarnessHooks>,
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
