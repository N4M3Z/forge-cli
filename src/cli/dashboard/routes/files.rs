use axum::extract::{Path, State};
use axum::response::{Html, IntoResponse};
use std::path::PathBuf;

use super::AppState;
use super::shared::{display_path, not_found};
use crate::cli::dashboard::scan;
use crate::cli::dashboard::templates;

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

/// Forge-cli's own config files surfaced from `~/.config/forge`. Per-artifact
/// config there (forensic.yaml, avatar.yaml, ...) holds private identifiers and
/// is deliberately excluded; only this allowlist is rendered.
const FORGE_CONFIG_FILES: &[&str] = &[
    "config.yaml",
    "config.yml",
    "config.toml",
    "config.json",
    "watchlist.yaml",
];

/// Forge config files at the scanned root plus the allowlisted forge-cli config
/// files in `~/.config/forge`, in a stable order so index-based detail routing
/// stays valid across requests.
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
                .filter(|name| {
                    FORGE_CONFIG_FILES
                        .iter()
                        .any(|allowed| name.eq_ignore_ascii_case(allowed))
                })
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

pub(super) async fn settings_page(State(app): State<AppState>) -> impl IntoResponse {
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

pub(super) async fn settings_detail(
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

pub(super) async fn hooks_page(State(app): State<AppState>) -> impl IntoResponse {
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

pub(super) async fn hook_detail(
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

pub(super) async fn schema_file_detail(
    State(app): State<AppState>,
    Path((group_index, index)): Path<(usize, usize)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let allowed = scan::active_repo_names(&state.view.modules, &app.root);
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
