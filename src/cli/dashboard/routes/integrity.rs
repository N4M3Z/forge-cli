use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};

use super::AppState;
use crate::cli::dashboard::scan;
use crate::cli::dashboard::templates;

pub(super) async fn provenance_page(State(app): State<AppState>) -> impl IntoResponse {
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

pub(super) async fn adrs_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let template = templates::AdrsTemplate {
        tab: "adrs",
        version: &state.version,
        view: &state.view,
    };
    Html(template.to_string())
}

/// Variant coverage grid at `/variants` — artifacts with qualifier overrides
/// against the targets they cover.
pub(super) async fn variants_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let coverage = commands::services::builders::build_variant_coverage(&state.view);
    let template = templates::VariantsTemplate {
        tab: "variants",
        version: &state.version,
        coverage,
    };
    Html(template.to_string())
}

/// ADR detail at `/adr/{repo}/{id}`. Builds a synthetic artifact so the rich
/// detail view (preview/code, frontmatter, git history, sidecar) is reused.
pub(super) async fn adr_detail(
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
    let artifact = scan::build_adr_artifact(adr, &state.local_repos);
    let provenance_raw = scan::read_source_sidecar(
        &adr.source_uri,
        Some(&adr.relative_path),
        &state.local_repos,
    )
    .unwrap_or_default();
    let dep_links =
        commands::services::builders::resolve_dep_links(&state.view, artifact.adoption.as_ref());
    let template = templates::ArtifactDetailTemplate {
        tab: "adrs",
        version: &state.version,
        binary_hash: &state.binary_hash,
        artifact: &artifact,
        module_name: &adr.repo,
        module_source_uri: super::shared::http_uri(&adr.source_uri),
        deploy_groups: Vec::new(),
        provenance_raw,
        diff_deployed: String::new(),
        diff_source_at_deploy: String::new(),
        dep_links,
        schema_applies: super::files::schema_label_for(&adr.source_uri, "adr", &state.local_repos),
    };
    Html(template.to_string()).into_response()
}
