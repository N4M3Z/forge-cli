use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse};
use serde::Deserialize;

use super::AppState;
use super::PAGE_SIZE;
use crate::cli::dashboard::templates;
use commands::view::ArtifactView;

#[derive(Deserialize)]
pub(super) struct SearchParams {
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
pub(super) struct OverviewParams {
    #[serde(default)]
    view: String,
    #[serde(default)]
    primary: String,
    #[serde(default)]
    density: String,
}

pub(super) async fn overview(
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

pub(super) async fn chrome(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let template = templates::ChromeTemplate {
        view: &state.view,
        scanned_at: &state.scanned_at,
    };
    Html(template.to_string())
}

pub(super) async fn modules_page(State(app): State<AppState>) -> impl IntoResponse {
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

pub(super) async fn module_detail(
    State(app): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
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

pub(super) async fn search(
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
