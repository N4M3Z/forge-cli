use axum::extract::{Path, Query, State};
use axum::response::{Html, IntoResponse};
use serde::Deserialize;
use std::path::PathBuf;

use super::shared::{canonical_source, display_path, http_uri, strip_extension};
use super::{AppState, DashboardState};
use crate::cli::dashboard::scan;
use crate::cli::dashboard::server;
use crate::cli::dashboard::templates;
use commands::services::builders::{group_deployments, resolve_dep_links};
use commands::view::{ArtifactView, DashboardView, ModuleView, ProvenanceArtifact};

/// Artifact detail: `/artifact/{module}/{kind}/{name}`. The module qualifier
/// disambiguates the same artifact present in more than one module (e.g. an
/// adopted copy).
pub(super) async fn artifact_detail_in_module(
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
        .filter(|prov| {
            canonical_source(&prov.source_uri) == canonical_source(&module_view.source_uri)
        })
        .flat_map(|prov| &prov.artifacts)
        .filter(|entry| strip_extension(&entry.deployed_path) == artifact_stem)
        .collect();
    let deploy_groups = group_deployments(&provenance_entries);
    let provenance_raw = scan::read_source_sidecar(
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
            scan::source_at_deploy(
                &entry.input_sha,
                &module_view.source_uri,
                &artifact.relative_path,
                &state.local_repos,
            )
        })
        .unwrap_or_default();
    let dep_links = resolve_dep_links(&state.view, artifact.adoption.as_ref());
    let schema_applies =
        super::files::schema_label_for(&module_view.source_uri, &artifact.kind, &state.local_repos);
    let template = templates::ArtifactDetailTemplate {
        tab: "artifact",
        version: &state.version,
        binary_hash: &state.binary_hash,
        artifact,
        module_name: &module_view.name,
        module_source_uri: http_uri(&module_view.source_uri),
        deploy_groups,
        provenance_raw,
        diff_deployed,
        diff_source_at_deploy,
        dep_links,
        schema_applies,
    };
    Html(template.to_string()).into_response()
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

pub(super) async fn companion_detail(
    State(app): State<AppState>,
    Path((module_name, parent, name)): Path<(String, String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let found = state
        .view
        .modules
        .iter()
        .filter(|module| module.name == module_name)
        .find_map(|module| {
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
        .filter(|prov| canonical_source(&prov.source_uri) == canonical_source(&source_uri))
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
        module: module_name,
        relative_path: companion.relative_path.clone(),
        source_path: companion.relative_path.clone(),
        description: companion.description.clone(),
        content_preview: String::new(),
        content_body: companion.content_body.clone(),
        raw_source: companion.raw_source.clone(),
        metadata: Vec::new(),
        providers,
        git_log: scan::git_log_for_artifact(
            &source_uri,
            Some(&companion.relative_path),
            &state.local_repos,
        ),
        adoption: scan::read_source_adoption(
            &source_uri,
            Some(&companion.relative_path),
            &state.local_repos,
        ),
        sidecar_warning: String::new(),
        broken_refs: Vec::new(),
        age_days: None,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
        vcs: None,
    };
    let companion_label = format!("{parent} / {name}");
    let provenance_raw = scan::read_source_sidecar(
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
        module_source_uri: http_uri(&source_uri),
        deploy_groups,
        provenance_raw,
        diff_deployed: String::new(),
        diff_source_at_deploy: String::new(),
        dep_links: resolve_dep_links(&state.view, artifact.adoption.as_ref()),
        schema_applies: String::new(),
    };
    Html(template.to_string()).into_response()
}

pub(super) async fn refresh(State(app): State<AppState>) -> impl IntoResponse {
    match server::build_state(&app.root) {
        Ok(new_state) => {
            let mut state = app.shared.write().await;
            *state = new_state;
            (
                axum::http::StatusCode::OK,
                [("HX-Refresh", "true")],
                "rescanned",
            )
                .into_response()
        }
        Err(error) => {
            eprintln!("refresh failed: {error}");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "rescan failed; see server log",
            )
                .into_response()
        }
    }
}

pub(super) async fn deployed(
    State(app): State<AppState>,
    Path((module, harness, path)): Path<(String, String, String)>,
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
    let source = read_current_source(&state, &module, &path);
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
fn read_current_source(state: &DashboardState, module_name: &str, deployed_path: &str) -> String {
    let stem = strip_extension(deployed_path);
    let Some((source_uri, source_path)) = state
        .view
        .modules
        .iter()
        .filter(|module| module.name == module_name)
        .find_map(|module| {
            module
                .artifacts
                .iter()
                .find(|artifact| strip_extension(&artifact.relative_path) == stem)
                .map(|artifact| (module.source_uri.clone(), artifact.relative_path.clone()))
        })
    else {
        return String::new();
    };
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo) = state.local_repos.get(normalized) else {
        return String::new();
    };
    let source_file = repo.join(source_path);
    match std::fs::read_to_string(&source_file) {
        Ok(content) => content,
        Err(error) => {
            eprintln!(
                "dashboard: cannot read source {}: {error}",
                source_file.display()
            );
            String::new()
        }
    }
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

/// Shows an artifact's source content at a specific commit via `git show`.
pub(super) async fn version_page(
    State(app): State<AppState>,
    Path((module, kind, name, sha)): Path<(String, String, String, String)>,
) -> impl IntoResponse {
    let state = app.shared.read().await;
    let located = find_source_location(&state.view, &module, &kind, &name);
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

#[derive(Deserialize)]
pub(super) struct EffectiveParams {
    #[serde(default)]
    qualifier: String,
}

/// Resolves an artifact's effective content for a `(provider, model)` qualifier
/// by merging its qualifier-directory variants over the base, mirroring
/// `commands::assemble::variants`. This shows the *authored* intent of the
/// PROV-0005 overlay; deploy currently resolves provider-only, so model-level
/// overlays are not yet what ships.
pub(super) async fn effective_page(
    State(app): State<AppState>,
    Path((module, kind, name)): Path<(String, String, String)>,
    Query(params): Query<EffectiveParams>,
) -> impl IntoResponse {
    use commands::assemble::variants;

    let state = app.shared.read().await;
    let Some((module_view, artifact)) = locate_artifact(&state.view, Some(&module), &kind, &name)
    else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!(
                "<p>Artifact {kind}/{name} not found in {module}.</p>"
            )),
        )
            .into_response();
    };

    let normalized = module_view.source_uri.trim_end_matches(".git");
    let Some(repo) = state.local_repos.get(normalized) else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html(format!("<p>No local checkout for {module}.</p>")),
        )
            .into_response();
    };

    let base_path = repo.join(&artifact.relative_path);
    let Some(source_directory) = base_path.parent() else {
        return (
            axum::http::StatusCode::NOT_FOUND,
            Html("<p>Artifact has no parent directory.</p>".to_string()),
        )
            .into_response();
    };
    let filename = artifact
        .relative_path
        .rsplit('/')
        .next()
        .unwrap_or(&artifact.relative_path);

    let qualifiers: Vec<String> = params
        .qualifier
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect();

    let base_content = match std::fs::read_to_string(&base_path) {
        Ok(content) => content,
        Err(error) => {
            eprintln!(
                "dashboard: cannot read base source {}: {error}",
                base_path.display()
            );
            format!(
                "(could not read source file {}: {error})\n",
                base_path.display()
            )
        }
    };
    let variant_path = variants::resolve(source_directory, filename, &qualifiers);

    let (merged, mode_label, variant_label) = resolve_effective_content(
        &base_content,
        variant_path.as_ref(),
        repo,
        &artifact.relative_path,
    );

    let title = format!("{}/{} · {}", kind, name, params.qualifier);
    let blurb = format!(
        "Effective content authored for target '{}' (merge mode: {}). \
         Source: {}. Deploy currently resolves provider-only, so model-level \
         overlays are authored here but not yet what forge ships.",
        params.qualifier, mode_label, variant_label
    );
    let files = vec![templates::ConfigFile {
        label: format!("{kind}/{name} → {}", params.qualifier),
        path: variant_label,
        language: "markdown".to_string(),
        content: merged,
    }];
    let template = templates::FilesTemplate {
        tab: "",
        title: &title,
        blurb: &blurb,
        version: &state.version,
        files,
    };
    Html(template.to_string()).into_response()
}

/// Merges an artifact's base content with its resolved variant (if any),
/// returning the rendered content, the merge-mode label, and a display label
/// for the variant path.
fn resolve_effective_content(
    base_content: &str,
    variant_path: Option<&PathBuf>,
    repo: &std::path::Path,
    relative_path: &str,
) -> (String, String, String) {
    use commands::assemble::variants::{self, Mode};
    match variant_path {
        Some(path) => {
            let variant_content = match std::fs::read_to_string(path) {
                Ok(content) => content,
                Err(error) => {
                    eprintln!("dashboard: cannot read variant {}: {error}", path.display());
                    String::new()
                }
            };
            let mode_field = scan::extract_frontmatter_field(&variant_content, "mode");
            let mode = Mode::parse(&mode_field);
            let label = path
                .strip_prefix(repo)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            (
                variants::apply(base_content, &variant_content, mode),
                if mode_field.is_empty() {
                    "replace".to_string()
                } else {
                    mode_field
                },
                label,
            )
        }
        None => (
            commands::parse::frontmatter_body(base_content).to_string(),
            "base".to_string(),
            format!("{relative_path} (no variant for this target)"),
        ),
    }
}

/// Finds an artifact's (or companion's) module source URI and source-relative
/// path. Top-level artifacts are matched first, then skill companions by name.
fn find_source_location(
    view: &DashboardView,
    module_name: &str,
    kind: &str,
    name: &str,
) -> Option<(String, String)> {
    let direct = view
        .modules
        .iter()
        .filter(|module| module.name == module_name)
        .find_map(|module| {
            module
                .artifacts
                .iter()
                .find(|artifact| artifact.kind == kind && artifact.name == name)
                .map(|artifact| (module.source_uri.clone(), artifact.relative_path.clone()))
        });
    if direct.is_some() {
        return direct;
    }
    view.modules
        .iter()
        .filter(|module| module.name == module_name)
        .find_map(|module| {
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
            variants: Vec::new(),
            source_path: String::new(),
            vcs: None,
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
            local_path: None,
            vcs: None,
            git_log: Vec::new(),
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
}
