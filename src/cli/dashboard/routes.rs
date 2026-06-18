use axum::Router;
use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::get;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use super::assets;
use super::server;
use super::templates;
use commands::view::{
    ArtifactView, DashboardView, DeployGroup, DeployHarness, ModuleView, ProvenanceArtifact,
};

pub struct DashboardState {
    pub view: DashboardView,
    pub provider_targets: Vec<(String, String)>,
    pub settings_filenames: Vec<String>,
    pub local_repos: std::collections::HashMap<String, PathBuf>,
    pub version: String,
    pub binary_hash: String,
    pub scanned_at: String,
}

type SharedState = Arc<RwLock<DashboardState>>;

#[derive(Clone)]
pub struct AppState {
    pub shared: SharedState,
    pub root: PathBuf,
}

pub fn router(shared: SharedState, root: PathBuf) -> Router {
    let app_state = AppState { shared, root };
    Router::new()
        .route("/", get(overview))
        .route("/chrome", get(chrome))
        .route("/repositories", get(modules_page))
        .route("/repositories/{name}", get(module_detail))
        .route(
            "/artifact/{module}/{kind}/{name}",
            get(artifact_detail_in_module),
        )
        .route("/companion/{parent}/{name}", get(companion_detail))
        .route("/provenance", get(provenance_page))
        .route("/adrs", get(adrs_page))
        .route("/adr/{repo}/{id}", get(adr_detail))
        .route("/search", get(search))
        .route("/refresh", get(refresh))
        .route("/deployed/{harness}/{*path}", get(deployed))
        .route("/version/{kind}/{name}/{sha}", get(version_page))
        .route("/settings", get(settings_page))
        .route("/settings/{harness}/{index}", get(settings_detail))
        .route("/hooks", get(hooks_page))
        .route("/hook/{harness}/{index}", get(hook_detail))
        .route("/config", get(config_page))
        .route("/config/{index}", get(config_detail))
        .route("/schemas", get(schemas_page))
        .route("/schemas/{group}/{index}", get(schema_file_detail))
        .route("/schema/{repo}/{kind}", get(schema_page))
        .route("/static/{*path}", get(assets::serve))
        .with_state(app_state)
        .layer(axum::middleware::from_fn(host_guard))
}

/// Rejects requests whose `Host` header is not a loopback name. Without this,
/// a remote page can use DNS rebinding to reach the dashboard on 127.0.0.1 and
/// read local artifact/config content. Browsers always send `Host`; a missing
/// or non-loopback host is refused.
async fn host_guard(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let host_allowed = request
        .headers()
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
        .map(host_name)
        .is_some_and(|name| matches!(name, "127.0.0.1" | "localhost" | "forge.localhost" | "::1"));
    if host_allowed {
        next.run(request).await
    } else {
        (
            axum::http::StatusCode::FORBIDDEN,
            "forbidden: dashboard only serves loopback hosts",
        )
            .into_response()
    }
}

/// Extracts the hostname from a `Host` header value, dropping any port. Handles
/// bracketed IPv6 literals (`[::1]:40000` -> `::1`).
fn host_name(host: &str) -> &str {
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    host.split(':').next().unwrap_or(host)
}

const PAGE_SIZE: usize = 48;

#[derive(Deserialize)]
struct SearchParams {
    #[serde(default)]
    query: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    module: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    sort: String,
    #[serde(default = "default_page")]
    page: usize,
}

fn default_page() -> usize {
    1
}

#[derive(Deserialize)]
struct OverviewParams {
    #[serde(default)]
    view: String,
    #[serde(default)]
    primary: String,
    #[serde(default)]
    density: String,
}

async fn overview(
    State(app): State<AppState>,
    Query(params): Query<OverviewParams>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let layout = if params.view == "matrix" {
        "matrix"
    } else {
        "nested"
    };
    let primary = if params.primary == "module" {
        "module"
    } else {
        "kind"
    };
    let density = if params.density == "compact" {
        "compact"
    } else {
        "comfortable"
    };
    let nested = if layout == "nested" {
        templates::build_nested(&state.view, primary)
    } else {
        Vec::new()
    };
    let matrix = (layout == "matrix").then(|| templates::build_matrix(&state.view));
    let template = templates::OverviewTemplate {
        tab: "overview",
        version: &state.version,
        view: &state.view,
        scanned_at: &state.scanned_at,
        layout,
        primary,
        density,
        nested,
        matrix,
    };
    Html(template.to_string())
}

async fn chrome(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let template = templates::ChromeTemplate {
        view: &state.view,
        scanned_at: &state.scanned_at,
    };
    Html(template.to_string())
}

async fn modules_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let first_module = state
        .view
        .modules
        .first()
        .map(|module| module.name.as_str());
    let detail =
        first_module.and_then(|name| state.view.modules.iter().find(|module| module.name == name));
    let selected = first_module.unwrap_or_default();
    let template = templates::ModulesTemplate {
        tab: "repositories",
        version: &state.version,
        view: &state.view,
        selected_module: selected,
        detail,
    };
    Html(template.to_string())
}

async fn module_detail(State(app): State<AppState>, Path(name): Path<String>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let module = state.view.modules.iter().find(|module| module.name == name);
    match module {
        Some(module) => {
            let template = templates::ModuleDetailTemplate { module };
            Html(template.to_string()).into_response()
        }
        None => (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>Module '{name}' not found.</p>")),
        )
            .into_response(),
    }
}

/// Artifact detail: `/artifact/{module}/{kind}/{name}`. The module qualifier
/// disambiguates the same artifact present in more than one module (e.g. an
/// adopted copy).
async fn artifact_detail_in_module(
    State(app): State<AppState>,
    Path((module, kind, name)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    render_artifact(&state, &app.root, Some(&module), &kind, &name)
}

/// Finds an artifact and its owning module, optionally restricted to a named
/// module. With no module the first match wins (legacy unqualified links).
fn locate_artifact<'a>(
    view: &'a DashboardView,
    module: Option<&str>,
    kind: &str,
    name: &str,
) -> Option<(&'a ModuleView, &'a ArtifactView)> {
    view.modules
        .iter()
        .filter(|candidate| module.is_none_or(|wanted| candidate.name == wanted))
        .find_map(|candidate| {
            candidate
                .artifacts
                .iter()
                .find(|artifact| artifact.kind == kind && artifact.name == name)
                .map(|artifact| (candidate, artifact))
        })
}

fn render_artifact(
    state: &DashboardState,
    root: &std::path::Path,
    module: Option<&str>,
    kind: &str,
    name: &str,
) -> axum::response::Response {
    let Some((module_view, artifact)) = locate_artifact(&state.view, module, kind, name) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>Artifact {kind}/{name} not found.</p>")),
        )
            .into_response();
    };
    let artifact_stem = strip_extension(&artifact.relative_path);
    let provenance_entries: Vec<&ProvenanceArtifact> = state
        .view
        .provenance
        .iter()
        .flat_map(|prov| &prov.artifacts)
        .filter(|entry| strip_extension(&entry.deployed_path) == artifact_stem)
        .collect();
    let deploy_groups = group_deployments(&provenance_entries);
    let provenance_raw = super::scan::read_source_sidecar(
        &module_view.source_uri,
        Some(&artifact.relative_path),
        &state.local_repos,
    )
    .or_else(|| read_deployed_sidecar(state, root, &provenance_entries))
    .unwrap_or_default();
    let diff_deployed = primary_deployed_content(state, root, &provenance_entries);
    let diff_source_at_deploy = provenance_entries
        .first()
        .filter(|entry| !entry.input_sha.is_empty())
        .and_then(|entry| {
            super::scan::source_at_deploy(
                &entry.input_sha,
                &module_view.source_uri,
                &artifact.relative_path,
                &state.local_repos,
            )
        })
        .unwrap_or_default();
    let dep_links = resolve_dep_links(&state.view, artifact.adoption.as_ref());
    let schema_applies =
        schema_label_for(&module_view.source_uri, &artifact.kind, &state.local_repos);
    let template = templates::ArtifactDetailTemplate {
        tab: "artifact",
        version: &state.version,
        binary_hash: &state.binary_hash,
        artifact,
        module_name: &module_view.name,
        module_source_uri: &module_view.source_uri,
        deploy_groups,
        provenance_raw,
        diff_deployed,
        diff_source_at_deploy,
        dep_links,
        schema_applies,
    };
    Html(template.to_string()).into_response()
}

/// Resolves each adoption dependency to the module containing a skill of that
/// name (first match), so the dep chip links to the correct copy. An unresolved
/// dependency gets an empty module and renders as plain text (no link, no 404).
fn resolve_dep_links(
    view: &DashboardView,
    adoption: Option<&commands::view::Adoption>,
) -> Vec<templates::DepLink> {
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
                    candidate
                        .artifacts
                        .iter()
                        .any(|art| art.kind == "skills" && art.name == dependency.name)
                })
                .map_or_else(String::new, |candidate| candidate.name.clone());
            templates::DepLink {
                name: dependency.name.clone(),
                uri: dependency.uri.clone(),
                module,
            }
        })
        .collect()
}

/// Falls back to the deployed `assemble/v1` sidecar (at the target's
/// `.provenance/` directory) when an artifact has no source-side adoption
/// sidecar, so the Provenance "Sidecar" view is available for authored
/// artifacts too. Returns `None` when no deployed sidecar is found.
fn read_deployed_sidecar(
    state: &DashboardState,
    root: &std::path::Path,
    entries: &[&ProvenanceArtifact],
) -> Option<String> {
    let entry = entries.first()?;
    let provider_dir = state
        .provider_targets
        .iter()
        .find(|(name, _)| *name == entry.harness)
        .map(|(_, dir)| dir.clone())?;
    let deployed = std::path::Path::new(&entry.deployed_path);
    let stem = deployed.file_stem()?.to_string_lossy();
    let parent = deployed
        .parent()
        .map_or_else(String::new, |dir| format!("{}/", dir.display()));
    let sidecar_rel = format!("{parent}.provenance/{stem}.yaml");
    read_deployed_file(root, &provider_dir, &sidecar_rel).map(|(_, content)| content)
}

/// Reads the content of the artifact's primary deployed copy (first provenance
/// entry), for the artifact-page "vs deployed" diff. Empty if not deployed.
fn primary_deployed_content(
    state: &DashboardState,
    root: &std::path::Path,
    entries: &[&ProvenanceArtifact],
) -> String {
    let Some(entry) = entries.first() else {
        return String::new();
    };
    let Some(provider_dir) = state
        .provider_targets
        .iter()
        .find(|(name, _)| *name == entry.harness)
        .map(|(_, dir)| dir.clone())
    else {
        return String::new();
    };
    read_deployed_file(root, &provider_dir, &entry.deployed_path)
        .map(|(_, content)| content)
        .unwrap_or_default()
}

async fn companion_detail(
    State(app): State<AppState>,
    Path((parent, name)): Path<(String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let found = state.view.modules.iter().find_map(|module| {
        module
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == "skills" && artifact.name == parent)
            .and_then(|skill| skill.companions.iter().find(|comp| comp.name == name))
            .map(|comp| (module.source_uri.clone(), comp.clone()))
    });
    let Some((source_uri, companion)) = found else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>Companion {parent}/{name} not found.</p>")),
        )
            .into_response();
    };

    let stem = strip_extension(&companion.relative_path);
    let provenance_entries: Vec<&ProvenanceArtifact> = state
        .view
        .provenance
        .iter()
        .flat_map(|prov| &prov.artifacts)
        .filter(|entry| strip_extension(&entry.deployed_path) == stem)
        .collect();
    let deploy_groups = group_deployments(&provenance_entries);

    let mut providers = std::collections::BTreeMap::new();
    for entry in &provenance_entries {
        providers.insert(
            entry.harness.clone(),
            commands::view::ProviderStatus {
                status: if entry.verified {
                    commands::manifest::FileStatus::Unchanged
                } else {
                    commands::manifest::FileStatus::Modified
                },
                fingerprint: Some(entry.deployed_sha.clone()),
            },
        );
    }

    let artifact = ArtifactView {
        name: companion.name.clone(),
        kind: "skills".to_string(),
        module: String::new(),
        relative_path: companion.relative_path.clone(),
        description: companion.description.clone(),
        content_preview: String::new(),
        content_body: companion.content_body.clone(),
        raw_source: companion.raw_source.clone(),
        metadata: Vec::new(),
        providers,
        git_log: super::scan::git_log_for_artifact(
            &source_uri,
            Some(&companion.relative_path),
            &state.local_repos,
        ),
        adoption: super::scan::read_source_adoption(
            &source_uri,
            Some(&companion.relative_path),
            &state.local_repos,
        ),
        sidecar_warning: String::new(),
        broken_refs: Vec::new(),
        age_days: None,
        module_tint: 0,
        companions: Vec::new(),
    };
    let companion_label = format!("{parent} / {name}");
    let provenance_raw = super::scan::read_source_sidecar(
        &source_uri,
        Some(&companion.relative_path),
        &state.local_repos,
    )
    .unwrap_or_default();
    let template = templates::ArtifactDetailTemplate {
        tab: "artifact",
        version: &state.version,
        binary_hash: &state.binary_hash,
        artifact: &artifact,
        module_name: &companion_label,
        module_source_uri: &source_uri,
        deploy_groups,
        provenance_raw,
        diff_deployed: String::new(),
        diff_source_at_deploy: String::new(),
        dep_links: resolve_dep_links(&state.view, artifact.adoption.as_ref()),
        schema_applies: String::new(),
    };
    Html(template.to_string()).into_response()
}

async fn search(
    State(app): State<AppState>,
    Query(params): Query<SearchParams>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let query_lower = params.query.to_lowercase();
    let mut matched: Vec<(&ArtifactView, &str)> = state
        .view
        .modules
        .iter()
        .filter(|module| params.module.is_empty() || module.name == params.module)
        .flat_map(|module| {
            module
                .artifacts
                .iter()
                .map(move |artifact| (artifact, module.name.as_str()))
        })
        .filter(|(artifact, _)| {
            if !params.kind.is_empty() && artifact.kind != params.kind {
                return false;
            }
            if !params.status.is_empty() && !matches_status(artifact, &params.status) {
                return false;
            }
            if query_lower.is_empty() {
                return true;
            }
            artifact.matches_query(&query_lower)
        })
        .collect();

    sort_results(&mut matched, &params.sort);

    let total = matched.len();
    let total_pages = total.div_ceil(PAGE_SIZE);
    let page = params.page.max(1).min(total_pages.max(1));
    let start = (page - 1) * PAGE_SIZE;
    let paged: Vec<&ArtifactView> = matched
        .into_iter()
        .skip(start)
        .take(PAGE_SIZE)
        .map(|(artifact, _)| artifact)
        .collect();
    let groups = commands::view::group_by_kind(&paged);

    let is_htmx = headers.contains_key("hx-request");
    if is_htmx {
        let template = templates::SearchResultsTemplate {
            groups,
            page,
            total_pages,
            total,
            query: &params.query,
            kind: &params.kind,
            module: &params.module,
            status: &params.status,
            sort: &params.sort,
        };
        return Html(template.to_string());
    }
    let template = templates::SearchPageTemplate {
        tab: "search",
        version: &state.version,
        view: &state.view,
        scanned_at: &state.scanned_at,
        groups,
        query: &params.query,
        kind: &params.kind,
        page,
        total_pages,
        total,
        module: &params.module,
        status: &params.status,
        sort: &params.sort,
    };
    Html(template.to_string())
}

async fn provenance_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let mut verified = 0;
    let mut total = 0;
    let mut orphans = 0;
    for record in &state.view.provenance {
        verified += record.verified;
        total += record.total;
        orphans += record.orphans.len();
    }
    let mut problems = Vec::new();
    let mut broken = 0;
    for module in &state.view.modules {
        for artifact in &module.artifacts {
            let issue = artifact.overall_status();
            if issue == "stale" || issue == "modified" {
                problems.push(templates::IntegrityProblem {
                    kind: artifact.kind.clone(),
                    name: artifact.name.clone(),
                    module: module.name.clone(),
                    issue: issue.to_string(),
                    detail: if issue == "stale" {
                        "source moved since deploy".to_string()
                    } else {
                        "deployed file edited".to_string()
                    },
                });
            }
            if artifact.has_broken_refs() {
                broken += 1;
                let count = artifact.broken_refs.len();
                problems.push(templates::IntegrityProblem {
                    kind: artifact.kind.clone(),
                    name: artifact.name.clone(),
                    module: module.name.clone(),
                    issue: "broken-refs".to_string(),
                    detail: format!(
                        "{count} broken reference{}",
                        if count == 1 { "" } else { "s" }
                    ),
                });
            }
        }
    }
    problems.sort_by(|a, b| a.issue.cmp(&b.issue).then_with(|| a.name.cmp(&b.name)));
    let template = templates::ProvenanceTemplate {
        tab: "provenance",
        version: &state.version,
        verified,
        total,
        stale: state.view.summary.stale,
        modified: state.view.summary.modified,
        drift: total.saturating_sub(verified),
        orphans,
        broken,
        problems,
    };
    Html(template.to_string())
}

async fn adrs_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let template = templates::AdrsTemplate {
        tab: "adrs",
        version: &state.version,
        view: &state.view,
    };
    Html(template.to_string())
}

/// ADR detail at `/adr/{repo}/{id}`. Builds a synthetic artifact so the rich
/// detail view (preview/code, frontmatter, git history, sidecar) is reused.
async fn adr_detail(
    State(app): State<AppState>,
    Path((repo, id)): Path<(String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let Some(adr) = state
        .view
        .adrs
        .iter()
        .find(|adr| adr.repo == repo && adr.id == id)
    else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>ADR {repo}/{id} not found.</p>")),
        )
            .into_response();
    };
    let artifact = super::scan::build_adr_artifact(adr, &state.local_repos);
    let provenance_raw = super::scan::read_source_sidecar(
        &adr.source_uri,
        Some(&adr.relative_path),
        &state.local_repos,
    )
    .unwrap_or_default();
    let dep_links = resolve_dep_links(&state.view, artifact.adoption.as_ref());
    let template = templates::ArtifactDetailTemplate {
        tab: "adrs",
        version: &state.version,
        binary_hash: &state.binary_hash,
        artifact: &artifact,
        module_name: &adr.repo,
        module_source_uri: &adr.source_uri,
        deploy_groups: Vec::new(),
        provenance_raw,
        diff_deployed: String::new(),
        diff_source_at_deploy: String::new(),
        dep_links,
        schema_applies: schema_label_for(&adr.source_uri, "adr", &state.local_repos),
    };
    Html(template.to_string()).into_response()
}

/// Groups deployment provenance entries by target location, so the graph
/// shows one node per directory (expandable) rather than a flat harness list.
fn group_deployments(entries: &[&ProvenanceArtifact]) -> Vec<DeployGroup> {
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

async fn refresh(State(app): State<AppState>) -> impl IntoResponse {
    match server::build_state(&app.root) {
        Ok(new_state) => {
            let mut state = app.shared.write().await;
            *state = new_state;
        }
        Err(error) => {
            eprintln!("refresh failed: {error}");
        }
    }
    Redirect::to("/")
}

/// Sorts matched artifacts in place. `recent` (default) orders by latest commit
/// date descending; `name` / `module` lexically; `size` by total lines
/// descending; `age` by oldest commit first; `staleness` by worst signal first
/// (broken references, then modified, then stale).
fn sort_results(matched: &mut [(&ArtifactView, &str)], sort: &str) {
    match sort {
        "name" => matched.sort_by(|a, b| a.0.name.cmp(&b.0.name)),
        "module" => matched.sort_by(|a, b| a.1.cmp(b.1).then_with(|| a.0.name.cmp(&b.0.name))),
        "size" => matched.sort_by(|a, b| {
            b.0.total_lines()
                .cmp(&a.0.total_lines())
                .then_with(|| a.0.name.cmp(&b.0.name))
        }),
        "age" => matched.sort_by(|a, b| {
            // Oldest first; artifacts with no git history sort last.
            match (a.0.age_days, b.0.age_days) {
                (Some(left), Some(right)) => right.cmp(&left).then_with(|| a.0.name.cmp(&b.0.name)),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.0.name.cmp(&b.0.name),
            }
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

/// Whether an artifact matches a status filter value. `attention` is a composite
/// matching anything needing review (broken references, modified, or stale);
/// other values match `overall_status` exactly.
fn matches_status(artifact: &ArtifactView, status: &str) -> bool {
    if status == "attention" {
        return artifact.has_broken_refs()
            || matches!(artifact.overall_status(), "modified" | "stale");
    }
    artifact.overall_status() == status
}

/// Strips a `.md` or `.toml` extension, so the same artifact deployed as
/// `SKILL.md` (claude) and `SKILL.toml` (codex) compares equal.
fn strip_extension(path: &str) -> &str {
    path.strip_suffix(".md")
        .or_else(|| path.strip_suffix(".toml"))
        .unwrap_or(path)
}

async fn deployed(
    State(app): State<AppState>,
    Path((harness, path)): Path<(String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let provider_dir = state
        .provider_targets
        .iter()
        .find(|(name, _)| name == &harness)
        .map(|(_, dir)| dir.clone());
    let found = provider_dir
        .as_deref()
        .and_then(|dir| read_deployed_file(&app.root, dir, &path));
    let exists = found.is_some();
    let (full_path, raw_source) = found.unwrap_or_else(|| {
        let dir = provider_dir.unwrap_or_else(|| format!(".{harness}"));
        (format!("~/{dir}/{path}"), String::new())
    });
    let content_body = strip_frontmatter(&raw_source);
    let source = read_current_source(&state, &path);
    let template = templates::DeployedTemplate {
        tab: "",
        version: &state.version,
        harness: &harness,
        path: &full_path,
        exists,
        content_body,
        raw_source,
        source,
    };
    Html(template.to_string())
}

/// Reads the current source file for a deployed path, for source-vs-deployed
/// comparison. Matches the artifact by deployed path, resolves its repo.
fn read_current_source(state: &DashboardState, deployed_path: &str) -> String {
    let stem = strip_extension(deployed_path);
    let Some((source_uri, source_path)) = state.view.modules.iter().find_map(|module| {
        module
            .artifacts
            .iter()
            .find(|artifact| strip_extension(&artifact.relative_path) == stem)
            .map(|artifact| (module.source_uri.clone(), artifact.relative_path.clone()))
    }) else {
        return String::new();
    };
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo) = state.local_repos.get(normalized) else {
        return String::new();
    };
    std::fs::read_to_string(repo.join(source_path)).unwrap_or_default()
}

/// Reads a deployed file from `<base>/<provider_dir>/<path>`, checking the home
/// target then the scanned root. Returns `(display_path, content)`, or `None`
/// if the resolved path escapes the provider directory or the file is absent.
fn read_deployed_file(
    root: &std::path::Path,
    provider_dir: &str,
    path: &str,
) -> Option<(String, String)> {
    let home = dirs::home_dir();
    let mut bases: Vec<PathBuf> = Vec::new();
    if let Some(ref home) = home {
        bases.push(home.clone());
    }
    bases.push(root.to_path_buf());
    for base in bases {
        let harness_root = base.join(provider_dir);
        let candidate = harness_root.join(path);
        let (Ok(canonical_root), Ok(canonical_file)) =
            (harness_root.canonicalize(), candidate.canonicalize())
        else {
            continue;
        };
        if !canonical_file.starts_with(&canonical_root) {
            eprintln!(
                "dashboard: refused deployed path escaping {}: {}",
                canonical_root.display(),
                canonical_file.display()
            );
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&canonical_file) {
            let display = display_path(&canonical_file, home.as_deref());
            return Some((display, content));
        }
    }
    None
}

/// Renders an absolute path with the home directory abbreviated to `~`.
fn display_path(path: &std::path::Path, home: Option<&std::path::Path>) -> String {
    if let Some(home) = home
        && let Ok(rest) = path.strip_prefix(home)
    {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

/// Reads a file into a `ConfigFile` if it exists, abbreviating the path with `~`.
fn read_config_file(
    label: &str,
    path: &std::path::Path,
    language: &str,
) -> Option<templates::ConfigFile> {
    let content = std::fs::read_to_string(path).ok()?;
    Some(templates::ConfigFile {
        label: label.to_string(),
        path: display_path(path, dirs::home_dir().as_deref()),
        language: language.to_string(),
        content,
    })
}

/// Builds a grid card for a config file, pointing at its detail route.
fn file_card(file: &templates::ConfigFile, href: String) -> templates::FileCard {
    templates::FileCard {
        label: file.label.clone(),
        path: file.path.clone(),
        language: file.language.clone(),
        lines: file.content.lines().count(),
        href,
    }
}

/// Renders one config/settings/schema file as a read-only detail page.
fn render_single_file(
    tab: &'static str,
    file: templates::ConfigFile,
    version: &str,
) -> axum::response::Response {
    let title = file.label.clone();
    let blurb = file.path.clone();
    let template = templates::FilesTemplate {
        tab,
        title: &title,
        blurb: &blurb,
        version,
        files: vec![file],
    };
    Html(template.to_string()).into_response()
}

fn not_found(message: &str) -> axum::response::Response {
    (
        axum::http::StatusCode::NOT_FOUND,
        Html(format!("<p>{message}</p>")),
    )
        .into_response()
}

/// Forge config files at the scanned root plus everything in `~/.config/forge`,
/// in a stable order so index-based detail routing stays valid across requests.
fn collect_dashboard_config_files(root: &std::path::Path) -> Vec<templates::ConfigFile> {
    let mut files = Vec::new();
    for (label, name, lang) in [
        ("Module manifest", "module.yaml", "yaml"),
        ("Defaults", "defaults.yaml", "yaml"),
        ("Config override", "config.yaml", "yaml"),
        ("Consumer manifest", ".forge", "yaml"),
    ] {
        if let Some(file) = read_config_file(label, &root.join(name), lang) {
            files.push(file);
        }
    }
    if let Some(home) = dirs::home_dir() {
        let forge_dir = home.join(".config/forge");
        if let Ok(entries) = std::fs::read_dir(&forge_dir) {
            let mut names: Vec<_> = entries
                .flatten()
                .filter(|entry| entry.path().is_file())
                .map(|entry| entry.file_name().to_string_lossy().to_string())
                .collect();
            names.sort();
            for name in names {
                let is_toml = std::path::Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
                let lang = if is_toml { "toml" } else { "yaml" };
                if let Some(file) =
                    read_config_file("~/.config/forge", &forge_dir.join(&name), lang)
                {
                    files.push(file);
                }
            }
        }
    }
    files
}

async fn settings_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = settings_by_harness(
        &app.root,
        &state.provider_targets,
        &state.settings_filenames,
    )
    .into_iter()
    .map(|group| templates::FileCardGroup {
        cards: group
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| file_card(file, format!("/settings/{}/{index}", group.harness)))
            .collect(),
        title: group.harness,
    })
    .collect();
    let template = templates::FileGridTemplate {
        tab: "settings",
        title: "Settings",
        blurb: "Settings files detected per harness, across user and project scope.",
        version: &state.version,
        groups,
    };
    Html(template.to_string())
}

async fn settings_detail(
    State(app): State<AppState>,
    Path((harness, index)): Path<(String, usize)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = settings_by_harness(
        &app.root,
        &state.provider_targets,
        &state.settings_filenames,
    );
    let Some(group) = groups.into_iter().find(|group| group.harness == harness) else {
        return not_found("Unknown harness.");
    };
    let mut files = group.files;
    if index >= files.len() {
        return not_found("Unknown settings file.");
    }
    render_single_file("settings", files.remove(index), &state.version)
}

async fn hooks_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = hooks_by_harness(
        &app.root,
        &state.provider_targets,
        &state.settings_filenames,
    );
    let template = templates::HooksTemplate {
        tab: "hooks",
        version: &state.version,
        groups,
    };
    Html(template.to_string())
}

async fn hook_detail(
    State(app): State<AppState>,
    Path((harness, index)): Path<(String, usize)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = hooks_by_harness(
        &app.root,
        &state.provider_targets,
        &state.settings_filenames,
    );
    let Some(group) = groups.into_iter().find(|group| group.harness == harness) else {
        return not_found("Unknown harness.");
    };
    let mut hooks = group.hooks;
    if index >= hooks.len() {
        return not_found("Unknown hook.");
    }
    let hook = hooks.remove(index);
    let (wrapper, command) = unwrap_shell(&hook.command);
    let template = templates::HookDetailTemplate {
        tab: "hooks",
        version: &state.version,
        harness,
        event: hook.event,
        matcher: hook.matcher,
        source: hook.source,
        wrapper,
        command,
    };
    Html(template.to_string()).into_response()
}

/// Strips a `sh -c '<script>'` / `bash -c "<script>"` wrapper so the inner shell
/// highlights as code instead of one opaque string. Returns the wrapper label
/// (empty when none) and the command to display.
fn unwrap_shell(command: &str) -> (String, String) {
    let trimmed = command.trim();
    for program in ["sh", "bash", "zsh"] {
        for quote in ['\'', '"'] {
            let prefix = format!("{program} -c {quote}");
            if let Some(rest) = trimmed.strip_prefix(&prefix)
                && let Some(inner) = rest.strip_suffix(quote)
            {
                return (format!("{program} -c"), inner.to_string());
            }
        }
    }
    (String::new(), command.to_string())
}

/// Forge config files plus each harness's settings files, in a stable order so
/// the index-based detail route stays valid. Harness settings are prefixed with
/// the harness name to disambiguate user and project copies.
fn all_config_files(
    root: &std::path::Path,
    provider_targets: &[(String, String)],
    settings_filenames: &[String],
) -> Vec<templates::ConfigFile> {
    let mut files = collect_dashboard_config_files(root);
    for group in settings_by_harness(root, provider_targets, settings_filenames) {
        for mut file in group.files {
            file.label = format!("{} · {}", group.harness, file.label);
            files.push(file);
        }
    }
    files
}

async fn config_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let files = all_config_files(
        &app.root,
        &state.provider_targets,
        &state.settings_filenames,
    );
    let cards = files
        .iter()
        .enumerate()
        .map(|(index, file)| file_card(file, format!("/config/{index}")))
        .collect();
    let template = templates::FileGridTemplate {
        tab: "config",
        title: "Config",
        blurb: "Forge config plus per-harness settings, across user and project scope.",
        version: &state.version,
        groups: vec![templates::FileCardGroup {
            title: String::new(),
            cards,
        }],
    };
    Html(template.to_string())
}

async fn config_detail(State(app): State<AppState>, Path(index): Path<usize>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let mut files = all_config_files(
        &app.root,
        &state.provider_targets,
        &state.settings_filenames,
    );
    if index >= files.len() {
        return not_found("Unknown config file.");
    }
    render_single_file("config", files.remove(index), &state.version)
}

async fn schemas_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let allowed = super::scan::active_repo_names(&state.view.modules, &app.root);
    let groups = schemas_by_source(
        &app.root,
        &state.provider_targets,
        &state.local_repos,
        &allowed,
    )
    .into_iter()
    .enumerate()
    .map(|(group_index, group)| templates::FileCardGroup {
        cards: group
            .files
            .iter()
            .enumerate()
            .map(|(index, file)| file_card(file, format!("/schemas/{group_index}/{index}")))
            .collect(),
        title: group.source,
    })
    .collect();
    let template = templates::FileGridTemplate {
        tab: "schemas",
        title: "Schemas & manifests",
        blurb: "Structure schemas (.mdschema) and deploy manifests (.manifest), by source.",
        version: &state.version,
        groups,
    };
    Html(template.to_string())
}

async fn schema_file_detail(
    State(app): State<AppState>,
    Path((group_index, index)): Path<(usize, usize)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let allowed = super::scan::active_repo_names(&state.view.modules, &app.root);
    let groups = schemas_by_source(
        &app.root,
        &state.provider_targets,
        &state.local_repos,
        &allowed,
    );
    let Some(group) = groups.into_iter().nth(group_index) else {
        return not_found("Unknown schema group.");
    };
    let mut files = group.files;
    if index >= files.len() {
        return not_found("Unknown schema file.");
    }
    render_single_file("schemas", files.remove(index), &state.version)
}

/// Collects `.mdschema` (per artifact kind + decisions) and `.manifest` files,
/// grouped by source: one group per repo (its schemas + module manifest) and
/// one per deploy target (its deployed `.manifest`).
fn schemas_by_source(
    root: &std::path::Path,
    provider_targets: &[(String, String)],
    local_repos: &std::collections::HashMap<String, PathBuf>,
    allowed: &std::collections::HashSet<String>,
) -> Vec<templates::SchemaGroup> {
    let mut groups = Vec::new();
    let mut repos: Vec<&PathBuf> = local_repos
        .values()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| allowed.contains(name.to_string_lossy().as_ref()))
        })
        .collect();
    repos.sort();
    for repo_path in repos {
        let mut files = Vec::new();
        for kind in ["skills", "agents", "rules"] {
            if let Some(file) = read_config_file(
                &format!("{kind}/.mdschema"),
                &repo_path.join(kind).join(".mdschema"),
                "yaml",
            ) {
                files.push(file);
            }
        }
        if let Some(file) = read_config_file(
            "docs/decisions/.mdschema",
            &repo_path.join("docs/decisions/.mdschema"),
            "yaml",
        ) {
            files.push(file);
        }
        if let Some(file) = read_config_file(".manifest", &repo_path.join(".manifest"), "yaml") {
            files.push(file);
        }
        if !files.is_empty() {
            let source = repo_path.file_name().map_or_else(
                || repo_path.display().to_string(),
                |name| name.to_string_lossy().to_string(),
            );
            groups.push(templates::SchemaGroup { source, files });
        }
    }
    let home = dirs::home_dir();
    for (_harness, target) in provider_targets {
        let mut bases: Vec<PathBuf> = Vec::new();
        if let Some(ref home) = home {
            bases.push(home.clone());
        }
        bases.push(root.to_path_buf());
        for base in bases {
            let provider_dir = base.join(target);
            if let Some(file) =
                read_config_file(".manifest", &provider_dir.join(".manifest"), "yaml")
            {
                groups.push(templates::SchemaGroup {
                    source: format!("deployed: {}", display_path(&provider_dir, home.as_deref())),
                    files: vec![file],
                });
            }
        }
    }
    groups
}

/// The artifact-kind directory holding a kind's `.mdschema`. ADRs live under
/// `docs/decisions`; every other kind under its own directory.
fn schema_dir(kind: &str) -> &str {
    if kind == "adr" {
        "docs/decisions"
    } else {
        kind
    }
}

/// The `<kind-dir>/.mdschema` label for an artifact, if that schema exists in
/// the artifact's module repo. Empty when no applicable schema is present.
fn schema_label_for(
    source_uri: &str,
    kind: &str,
    local_repos: &std::collections::HashMap<String, PathBuf>,
) -> String {
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo) = local_repos.get(normalized) else {
        return String::new();
    };
    let dir = schema_dir(kind);
    if repo.join(dir).join(".mdschema").is_file() {
        format!("{dir}/.mdschema")
    } else {
        String::new()
    }
}

/// Renders a single artifact-kind `.mdschema` at `/schema/{repo}/{kind}`.
async fn schema_page(
    State(app): State<AppState>,
    Path((repo, kind)): Path<(String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    // Allowlist the kind so the URL segment can never become a path-traversal
    // component in the join below, and so unknown kinds 404 instead of rendering
    // an empty page.
    if !matches!(kind.as_str(), "skills" | "agents" | "rules" | "adr") {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>Unknown artifact kind '{kind}'.</p>")),
        )
            .into_response();
    }
    let dir = schema_dir(&kind);
    let Some(repo_path) = state.local_repos.values().find(|path| {
        path.file_name()
            .is_some_and(|name| name.to_string_lossy() == repo)
    }) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>Unknown repo '{repo}'.</p>")),
        )
            .into_response();
    };
    let Some(file) = read_config_file(
        &format!("{dir}/.mdschema"),
        &repo_path.join(dir).join(".mdschema"),
        "yaml",
    ) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>No {dir}/.mdschema in {repo}.</p>")),
        )
            .into_response();
    };
    let title = format!("{repo} · {dir}/.mdschema");
    let template = templates::FilesTemplate {
        tab: "schemas",
        title: &title,
        blurb: "Structure schema applied to this artifact kind (read-only).",
        version: &state.version,
        files: vec![file],
    };
    Html(template.to_string()).into_response()
}

/// Shows an artifact's source content at a specific commit via `git show`.
async fn version_page(
    State(app): State<AppState>,
    Path((kind, name, sha)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let located = find_source_location(&state.view, &kind, &name);
    let content = located.as_ref().and_then(|(source_uri, source_path)| {
        let normalized = source_uri.trim_end_matches(".git");
        let repo = state.local_repos.get(normalized)?;
        git_show(repo, &sha, source_path)
    });
    let short: String = sha.chars().take(7).collect();
    let files = content.map_or_else(Vec::new, |body| {
        let path = located
            .map(|(_, source_path)| source_path)
            .unwrap_or_default();
        vec![templates::ConfigFile {
            label: format!("{kind}/{name} @ {short}"),
            path,
            language: "markdown".to_string(),
            content: body,
        }]
    });
    let template = templates::FilesTemplate {
        tab: "",
        title: "Version at commit",
        blurb: "Source content at this commit (git show), read-only.",
        version: &state.version,
        files,
    };
    Html(template.to_string())
}

/// Finds an artifact's (or companion's) module source URI and source-relative
/// path. Top-level artifacts are matched first, then skill companions by name.
fn find_source_location(view: &DashboardView, kind: &str, name: &str) -> Option<(String, String)> {
    let direct = view.modules.iter().find_map(|module| {
        module
            .artifacts
            .iter()
            .find(|artifact| artifact.kind == kind && artifact.name == name)
            .map(|artifact| (module.source_uri.clone(), artifact.relative_path.clone()))
    });
    if direct.is_some() {
        return direct;
    }
    view.modules.iter().find_map(|module| {
        module
            .artifacts
            .iter()
            .flat_map(|artifact| &artifact.companions)
            .find(|comp| comp.name == name)
            .map(|comp| (module.source_uri.clone(), comp.relative_path.clone()))
    })
}

/// Runs `git show {sha}:{path}` in a repo, returning the file content at that commit.
fn git_show(repo: &std::path::Path, sha: &str, path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["show", &format!("{sha}:{path}")])
        .current_dir(repo)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Settings/config files grouped per harness, auto-detected from each
/// provider's target directory (user scope then project scope). A harness with
/// no config files is omitted.
fn settings_by_harness(
    root: &std::path::Path,
    provider_targets: &[(String, String)],
    allowed: &[String],
) -> Vec<templates::HarnessFiles> {
    let home = dirs::home_dir();
    let mut groups = Vec::new();
    for (harness, target) in provider_targets {
        let mut files = Vec::new();
        if let Some(home) = &home {
            collect_config_files(&home.join(target), allowed, &mut files);
        }
        collect_config_files(&root.join(target), allowed, &mut files);
        if !files.is_empty() {
            groups.push(templates::HarnessFiles {
                harness: harness.clone(),
                files,
            });
        }
    }
    groups
}

/// Hooks grouped per harness, parsed from each harness's JSON settings files.
fn hooks_by_harness(
    root: &std::path::Path,
    provider_targets: &[(String, String)],
    allowed: &[String],
) -> Vec<templates::HarnessHooks> {
    let mut groups = Vec::new();
    for harness_files in settings_by_harness(root, provider_targets, allowed) {
        let mut hooks = Vec::new();
        for file in &harness_files.files {
            if file.language == "json" {
                parse_hooks(&file.content, &file.path, &mut hooks);
            }
        }
        if !hooks.is_empty() {
            groups.push(templates::HarnessHooks {
                harness: harness_files.harness,
                hooks,
            });
        }
    }
    groups
}

/// Reads allowlisted settings files at the top level of a directory, skipping
/// artifact subdirectories. The allowlist (from `dashboard.settings_files`)
/// keeps runtime state, caches, npm manifests, and credential files out of the
/// view. Files are added in sorted name order.
fn collect_config_files(
    dir: &std::path::Path,
    allowed: &[String],
    out: &mut Vec<templates::ConfigFile>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| allowed.iter().any(|known| name.eq_ignore_ascii_case(known)))
        .collect();
    names.sort();
    for name in names {
        if let Some(language) = extension_language(&name)
            && let Some(file) = read_config_file(&name, &dir.join(&name), language)
        {
            out.push(file);
        }
    }
}

/// Maps a filename extension to a highlight language for config files.
fn extension_language(name: &str) -> Option<&'static str> {
    let extension = std::path::Path::new(name).extension()?.to_str()?;
    match extension.to_ascii_lowercase().as_str() {
        "json" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        _ => None,
    }
}

/// Parses a settings.json `hooks` block into flat `HookEntry` rows.
fn parse_hooks(content: &str, source: &str, out: &mut Vec<templates::HookEntry>) {
    #[derive(serde::Deserialize)]
    struct Settings {
        #[serde(default)]
        hooks: std::collections::BTreeMap<String, Vec<HookMatcher>>,
    }
    #[derive(serde::Deserialize)]
    struct HookMatcher {
        #[serde(default)]
        matcher: String,
        #[serde(default)]
        hooks: Vec<HookCommand>,
    }
    #[derive(serde::Deserialize)]
    struct HookCommand {
        #[serde(default)]
        command: String,
    }
    let Ok(settings) = serde_json::from_str::<Settings>(content) else {
        return;
    };
    for (event, matchers) in settings.hooks {
        for matcher in matchers {
            for command in matcher.hooks {
                out.push(templates::HookEntry {
                    event: event.clone(),
                    matcher: matcher.matcher.clone(),
                    command: command.command,
                    source: source.to_string(),
                });
            }
        }
    }
}

fn strip_frontmatter(content: &str) -> String {
    let Some(rest) = content.strip_prefix("---") else {
        return content.to_string();
    };
    let Some(end) = rest.find("\n---") else {
        return content.to_string();
    };
    rest[end + 4..].trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_artifact(kind: &str, name: &str, module: &str) -> ArtifactView {
        ArtifactView {
            name: name.to_string(),
            kind: kind.to_string(),
            module: module.to_string(),
            relative_path: format!("{kind}/{name}.md"),
            description: String::new(),
            content_preview: String::new(),
            content_body: String::new(),
            raw_source: String::new(),
            metadata: Vec::new(),
            providers: std::collections::BTreeMap::new(),
            git_log: Vec::new(),
            adoption: None,
            sidecar_warning: String::new(),
            broken_refs: Vec::new(),
            age_days: None,
            module_tint: 0,
            companions: Vec::new(),
        }
    }

    fn make_module(name: &str, artifacts: Vec<ArtifactView>) -> ModuleView {
        ModuleView {
            name: name.to_string(),
            version: String::new(),
            description: String::new(),
            source_uri: format!("https://example.com/{name}"),
            is_target: false,
            artifacts,
        }
    }

    fn sample_view() -> DashboardView {
        DashboardView {
            modules: vec![
                make_module(
                    "forge-core",
                    vec![make_artifact("skills", "LearnFrom", "forge-core")],
                ),
                make_module(
                    "proton-agents",
                    vec![make_artifact("skills", "LearnFrom", "proton-agents")],
                ),
            ],
            summary: commands::view::StatusSummary::default(),
            provenance: Vec::new(),
            adrs: Vec::new(),
        }
    }

    #[test]
    fn locate_artifact_qualified_returns_named_module() {
        let view = sample_view();
        let (located_module, located_artifact) =
            locate_artifact(&view, Some("proton-agents"), "skills", "LearnFrom").unwrap();
        assert_eq!(located_module.name, "proton-agents");
        assert_eq!(located_artifact.module, "proton-agents");
    }

    #[test]
    fn locate_artifact_unqualified_returns_first_match() {
        let view = sample_view();
        let (located_module, _) = locate_artifact(&view, None, "skills", "LearnFrom").unwrap();
        assert_eq!(located_module.name, "forge-core");
    }

    #[test]
    fn locate_artifact_none_for_unknown() {
        let view = sample_view();
        assert!(locate_artifact(&view, None, "skills", "Missing").is_none());
    }

    #[test]
    fn host_name_strips_port_and_brackets() {
        assert_eq!(host_name("127.0.0.1:40000"), "127.0.0.1");
        assert_eq!(host_name("forge.localhost"), "forge.localhost");
        assert_eq!(host_name("[::1]:40000"), "::1");
    }
}
