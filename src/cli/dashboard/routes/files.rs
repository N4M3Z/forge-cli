use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};
use std::path::PathBuf;

use super::AppState;
use super::shared::not_found;
use crate::cli::dashboard::scan;
use crate::cli::dashboard::templates;
use commands::services::files as file_services;

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

pub(super) async fn settings_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = file_services::settings_by_harness(
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

pub(super) async fn settings_detail(
    State(app): State<AppState>,
    Path((harness, index)): Path<(String, usize)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = file_services::settings_by_harness(
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

pub(super) async fn hooks_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = file_services::hooks_by_harness(
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

pub(super) async fn hook_detail(
    State(app): State<AppState>,
    Path((harness, index)): Path<(String, usize)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let groups = file_services::hooks_by_harness(
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
    let (wrapper, command) = file_services::unwrap_shell(&hook.command);
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

/// Forge config files plus each harness's settings files, in a stable order so
/// the index-based detail route stays valid. Harness settings are prefixed with
/// the harness name to disambiguate user and project copies.
fn all_config_files(
    root: &std::path::Path,
    provider_targets: &[(String, String)],
    settings_filenames: &[String],
) -> Vec<templates::ConfigFile> {
    file_services::collect_dashboard_config_files(root, provider_targets, settings_filenames)
}

pub(super) async fn config_page(State(app): State<AppState>) -> impl IntoResponse {
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

pub(super) async fn config_detail(
    State(app): State<AppState>,
    Path(index): Path<usize>,
) -> impl IntoResponse {
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

pub(super) async fn schemas_page(State(app): State<AppState>) -> impl IntoResponse {
    let state = app.shared.read().await;
    let allowed = scan::active_repo_names(&state.view.modules, &app.root);
    let groups = file_services::schemas_by_source(
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

pub(super) async fn schema_file_detail(
    State(app): State<AppState>,
    Path((group_index, index)): Path<(usize, usize)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let allowed = scan::active_repo_names(&state.view.modules, &app.root);
    let groups = file_services::schemas_by_source(
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
pub(super) fn schema_label_for(
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
pub(super) async fn schema_page(
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
    let Some(file) = file_services::read_config_file(
        &format!("{dir}/.mdschema"),
        &repo_path.join(dir).join(".mdschema"),
        "yaml",
        dirs::home_dir().as_deref(),
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
