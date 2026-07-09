use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse};
use serde::Deserialize;

use super::AppState;
use super::PAGE_SIZE;
use crate::cli::dashboard::templates;
use commands::services::builders::{SearchFilters, search_results};
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
        commands::services::builders::build_nested(&state.view, primary)
    } else {
        Vec::new()
    };
    let matrix =
        (layout == "matrix").then(|| commands::services::builders::build_matrix(&state.view));
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
    let filters = SearchFilters {
        query: params.query.clone(),
        kind: params.kind.clone(),
        module: params.module.clone(),
        status: params.status.clone(),
        sort: params.sort.clone(),
    };
    let matched = search_results(&state.view, &filters);

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
