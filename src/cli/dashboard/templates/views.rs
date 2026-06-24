use askama::Template;
use commands::view::{ArtifactView, DashboardView, ModuleView};

/// Shared status + search bar loaded into card-list views via htmx.
#[derive(Template)]
#[template(path = "dashboard/partials/chrome.html")]
pub struct ChromeTemplate<'a> {
    pub view: &'a DashboardView,
    pub scanned_at: &'a str,
}

#[derive(Template)]
#[template(path = "dashboard/pages/overview.html")]
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

/// Coverage grid for model/harness variants: rows = artifacts that carry at
/// least one qualifier override, columns = the distinct qualifiers seen across
/// all such artifacts, cells = the merge mode for that (artifact, qualifier).
#[derive(Template)]
#[template(path = "dashboard/pages/variants.html")]
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

#[derive(Template)]
#[template(path = "dashboard/pages/adrs.html")]
pub struct AdrsTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub view: &'a DashboardView,
}

#[derive(Template)]
#[template(path = "dashboard/pages/modules.html")]
pub struct ModulesTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub view: &'a DashboardView,
    pub selected_module: &'a str,
    pub detail: Option<&'a ModuleView>,
}

#[derive(Template)]
#[template(path = "dashboard/pages/module_detail.html")]
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
#[template(path = "dashboard/pages/artifact_detail.html")]
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
#[template(path = "dashboard/partials/search_results.html")]
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
#[template(path = "dashboard/pages/search_page.html")]
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
#[template(path = "dashboard/pages/provenance.html")]
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
#[template(path = "dashboard/pages/deployed.html")]
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
#[template(path = "dashboard/pages/files.html")]
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
#[template(path = "dashboard/partials/file_grid.html")]
pub struct FileGridTemplate<'a> {
    pub tab: &'a str,
    pub title: &'a str,
    pub blurb: &'a str,
    pub version: &'a str,
    pub groups: Vec<FileCardGroup>,
}

/// One hook shown as a detail page (full command, source file).
#[derive(Template)]
#[template(path = "dashboard/pages/hook_detail.html")]
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
#[template(path = "dashboard/pages/hooks.html")]
pub struct HooksTemplate<'a> {
    pub tab: &'a str,
    pub version: &'a str,
    pub groups: Vec<HarnessHooks>,
}
