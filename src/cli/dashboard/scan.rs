//! Scan deployment targets and build a `DashboardView`.
//!
//! Forge artifacts can be deployed anywhere: `~/.claude/` (user-scope),
//! `./.claude/` (project-scope), or a custom `--target` path. The scanner
//! reads `.manifest` files from known provider directories at each target
//! and groups artifacts by their source module (via provenance `source_uri`).

use commands::error::{Error, ErrorKind};
use commands::manifest::{self, FileStatus, ManifestEntry};
use commands::provider::ContentKind;
use commands::view::{
    Adoption, Adr, ArtifactView, Companion, DashboardView, Dependency, GitCommit, ModuleView,
    ProvenanceArtifact, ProvenanceView, ProviderStatus, StatusSummary, Variant,
};
use regex::Regex;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Content kinds scanned, sourced from the shared `ContentKind` enum rather
/// than a local hardcoded list.
fn content_kinds() -> [&'static str; 3] {
    [
        ContentKind::Agents.as_str(),
        ContentKind::Rules.as_str(),
        ContentKind::Skills.as_str(),
    ]
}

pub fn build_view(root: &Path, providers: &[(String, String)]) -> Result<DashboardView, Error> {
    let targets = discover_targets(root, providers);
    if targets.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                "no deployed provider directories found at ~ or {}",
                root.display()
            ),
        ));
    }

    let local_repos = discover_local_repos(root);
    let mut modules_by_source: BTreeMap<String, ModuleView> = BTreeMap::new();
    let mut summary = StatusSummary::default();
    let mut pending_companions: Vec<PendingCompanion> = Vec::new();

    for target_base in &targets {
        scan_target(
            target_base,
            &mut modules_by_source,
            &mut summary,
            &local_repos,
            &mut pending_companions,
            providers,
        );
    }

    attach_companions(&mut modules_by_source, pending_companions);

    let mut modules: Vec<ModuleView> = modules_by_source.into_values().collect();

    for location in configured_locations() {
        let module_root = fs::canonicalize(&location).unwrap_or(location);
        if let Some(mut watched_module) = scan_source_module(&module_root) {
            watched_module.is_target = true;
            modules.push(watched_module);
        }
    }

    if modules.is_empty() {
        modules.push(ModuleView {
            name: "(no manifest)".to_string(),
            version: String::new(),
            description: "No .manifest files found at scanned targets".to_string(),
            source_uri: String::new(),
            is_target: false,
            artifacts: Vec::new(),
        });
    }

    let mut provenance = Vec::new();
    for target_base in &targets {
        collect_provenance(target_base, providers, &mut provenance);
    }

    let provider_names: Vec<String> = providers.iter().map(|(name, _)| name.clone()).collect();
    for (module_index, module) in modules.iter_mut().enumerate() {
        module.artifacts.sort_by(|a, b| a.name.cmp(&b.name));
        let repo = local_repos.get(module.source_uri.trim_end_matches(".git"));
        let tint = module_index % 8;
        for artifact in &mut module.artifacts {
            artifact.module.clone_from(&module.name);
            artifact.module_tint = tint;
            let (broken, age) = artifact_staleness(
                repo,
                &artifact.relative_path,
                &artifact.raw_source,
                artifact.latest_commit_date(),
            );
            artifact.broken_refs = broken;
            artifact.age_days = age;
            artifact.variants = repo
                .map(|repo| collect_variants(repo, &artifact.relative_path, &provider_names))
                .unwrap_or_default();
        }
    }

    let adrs = discover_adrs(&local_repos, &active_repo_names(&modules, root));

    Ok(DashboardView {
        modules,
        summary,
        provenance,
        adrs,
    })
}

/// Directory-name allowlist for ADRs and schemas: the active modules plus the
/// repo the dashboard runs in. Confines both to the same source set as the
/// rest of the dashboard, dropping ADRs from unrelated sibling repos.
pub fn active_repo_names(modules: &[ModuleView], root: &Path) -> HashSet<String> {
    let mut names: HashSet<String> = modules.iter().map(|module| module.name.clone()).collect();
    if let Some(root_name) = fs::canonicalize(root)
        .unwrap_or_else(|_| root.to_path_buf())
        .file_name()
    {
        names.insert(root_name.to_string_lossy().to_string());
    }
    names
}

/// Scans `docs/decisions/*.md` in the allowed repos for architecture decision
/// records. The filename `<ID> <Title>.md` yields id + title; status is read
/// from frontmatter when present. Repos are visited in path order so the
/// grouping is stable.
fn discover_adrs(local_repos: &HashMap<String, PathBuf>, allowed: &HashSet<String>) -> Vec<Adr> {
    let mut repos: Vec<(&String, &PathBuf)> = local_repos.iter().collect();
    repos.sort_by(|a, b| a.1.cmp(b.1));
    repos.retain(|(_, path)| {
        path.file_name()
            .is_some_and(|name| allowed.contains(name.to_string_lossy().as_ref()))
    });
    let mut adrs = Vec::new();
    for (source_uri, repo_path) in repos {
        let decisions = repo_path.join("docs/decisions");
        let Ok(entries) = fs::read_dir(&decisions) else {
            continue;
        };
        let repo_name = repo_path.file_name().map_or_else(
            || source_uri.clone(),
            |name| name.to_string_lossy().to_string(),
        );
        let mut names: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .filter(|name| {
                !name.starts_with('.')
                    && Path::new(name)
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("md"))
            })
            .collect();
        names.sort();
        for name in names {
            let stem = name.trim_end_matches(".md");
            let (id, title) = stem.split_once(' ').unwrap_or((stem, ""));
            let relative_path = format!("docs/decisions/{name}");
            let raw = fs::read_to_string(decisions.join(&name)).unwrap_or_default();
            let sidecar = resolve_sidecar(&decisions, Path::new(&relative_path))
                .and_then(|path| fs::read_to_string(path).ok());
            let (state, source) = adr_state(sidecar.as_deref(), &raw);
            adrs.push(Adr {
                id: id.to_string(),
                title: title.to_string(),
                status: extract_frontmatter_field(&raw, "status"),
                repo: repo_name.clone(),
                source_uri: source_uri.clone(),
                relative_path,
                state,
                source,
                summary: adr_summary(&raw),
            });
        }
    }
    adrs
}

/// Classifies an ADR from its sidecar: `authored` (no sidecar), `modified` (the
/// copy was edited since adoption), or `copied` (still matches what was copied).
/// Also returns the copied-from source label.
fn adr_state(sidecar: Option<&str>, current: &str) -> (String, String) {
    let Some(content) = sidecar else {
        return ("authored".to_string(), String::new());
    };
    let source = parse_adoption(content)
        .map(|adoption| adoption.source_label)
        .unwrap_or_default();
    let modified =
        recorded_subject_sha(content).is_some_and(|sha| manifest::content_sha256(current) != sha);
    let state = if modified { "modified" } else { "copied" };
    (state.to_string(), source)
}

/// One-paragraph preview for the ADR list: the `## Context` section's first
/// prose paragraph when present, otherwise the first prose paragraph after the
/// title. Headings and blank lines are skipped, backticks dropped, and the
/// result is truncated at a word boundary.
fn adr_summary(raw: &str) -> String {
    let body = strip_frontmatter(raw);
    let lines: Vec<&str> = body.lines().collect();
    let start = lines
        .iter()
        .position(|line| {
            line.starts_with("##")
                && line
                    .trim_start_matches('#')
                    .trim()
                    .eq_ignore_ascii_case("context")
        })
        .map_or(0, |index| index + 1);
    let mut paragraph = String::new();
    for line in &lines[start..] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if paragraph.is_empty() {
                continue;
            }
            break;
        }
        if !paragraph.is_empty() {
            paragraph.push(' ');
        }
        paragraph.push_str(trimmed);
    }
    truncate_summary(&paragraph.replace('`', ""), 260)
}

fn truncate_summary(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max).collect();
    match truncated.rfind(' ') {
        Some(index) => format!("{}…", &truncated[..index]),
        None => format!("{truncated}…"),
    }
}

/// Days since the most recent commit touching the artifact, parsed from a git
/// `%ai` date string (e.g. `2026-04-10 08:18:01 +0000`). `None` when there is no
/// history or the date does not parse.
fn commit_age_days(date_str: &str) -> Option<i64> {
    let parsed = chrono::DateTime::parse_from_str(date_str, "%Y-%m-%d %H:%M:%S %z").ok()?;
    Some((chrono::Utc::now() - parsed.with_timezone(&chrono::Utc)).num_days())
}

/// Unique intra-repo markdown link targets cited by `raw_source` that no longer
/// resolve on disk. Mirrors the validated resolver: code spans/fences stripped,
/// inline `](target)` + reference-style `[label]: target` extracted, only
/// relative path-shaped targets kept (URL-decoded), resolved against the
/// artifact's own directory and the repo root. External links are not checked.
fn broken_references(repo_root: &Path, artifact_dir: &Path, raw_source: &str) -> Vec<String> {
    let stripped = strip_code(raw_source);
    let mut broken = Vec::new();
    let mut seen = HashSet::new();
    for raw_target in link_targets(&stripped) {
        let Some(target) = normalize_reference(&raw_target) else {
            continue;
        };
        if !seen.insert(target.clone()) {
            continue;
        }
        if !reference_resolves(repo_root, artifact_dir, &target) {
            broken.push(target);
        }
    }
    broken
}

/// Removes fenced code blocks (line-delimited ```` ``` ````) and inline code
/// spans so example link syntax inside documentation is not mistaken for a real
/// reference.
fn strip_code(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        out.push_str(&strip_inline_code(line));
        out.push('\n');
    }
    out
}

/// Drops backtick-delimited inline code, keeping the prose between spans.
fn strip_inline_code(line: &str) -> String {
    line.split('`').step_by(2).collect::<String>()
}

/// Inline `](target)` and reference-style `[label]: target` link targets.
fn link_targets(text: &str) -> Vec<String> {
    static INLINE: OnceLock<Regex> = OnceLock::new();
    static REFDEF: OnceLock<Regex> = OnceLock::new();
    let inline = INLINE.get_or_init(|| Regex::new(r"\]\(([^)\s]+)\)").expect("valid regex"));
    let refdef =
        REFDEF.get_or_init(|| Regex::new(r"(?m)^\[[^\]]+\]:\s*(\S+)").expect("valid regex"));
    inline
        .captures_iter(text)
        .chain(refdef.captures_iter(text))
        .map(|capture| capture[1].to_string())
        .collect()
}

/// Link-target prefixes that are not local file references.
const EXTERNAL_REFERENCE_PREFIXES: [&str; 6] =
    ["http://", "https://", "mailto:", "tel:", "<", "//"];

/// Keeps only relative, path-shaped link targets (a slash or a file extension),
/// dropping anchors, external schemes, and prose. Returns the URL-decoded path.
fn normalize_reference(raw: &str) -> Option<String> {
    let target = raw.split('#').next().unwrap_or("");
    if target.is_empty() {
        return None;
    }
    if EXTERNAL_REFERENCE_PREFIXES
        .iter()
        .any(|prefix| target.starts_with(prefix))
    {
        return None;
    }
    let decoded = percent_decode(target);
    let has_extension = Path::new(&decoded)
        .extension()
        .is_some_and(|extension| !extension.is_empty());
    if !decoded.contains('/') && !has_extension {
        return None;
    }
    Some(decoded)
}

/// True when `target` resolves either relative to the artifact's directory or
/// relative to the repo root. `Path::exists` follows `..` and symlinks.
fn reference_resolves(repo_root: &Path, artifact_dir: &Path, target: &str) -> bool {
    artifact_dir.join(target).exists() || repo_root.join(target.trim_start_matches('/')).exists()
}

/// Decodes `%XX` percent-escapes (e.g. `%20` -> space) so encoded paths match
/// real filenames; leaves malformed escapes untouched.
fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
        {
            out.push((high << 4) | low);
            index += 3;
            continue;
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Computes reference integrity + age for one artifact, given its repo (if the
/// source is locally available) and source-relative path.
fn artifact_staleness(
    repo: Option<&PathBuf>,
    relative_path: &str,
    raw_source: &str,
    latest_commit_date: &str,
) -> (Vec<String>, Option<i64>) {
    let broken = repo.map_or_else(Vec::new, |repo_root| {
        let parent = Path::new(relative_path).parent();
        let artifact_dir =
            parent.map_or_else(|| repo_root.clone(), |relative| repo_root.join(relative));
        broken_references(repo_root, &artifact_dir, raw_source)
    });
    (broken, commit_age_days(latest_commit_date))
}

/// Builds a synthetic `ArtifactView` for an ADR so the artifact detail view can
/// render its content, frontmatter, git history, and any provenance sidecar.
/// ADRs are authored, not deployed, so providers and companions stay empty.
pub fn build_adr_artifact(adr: &Adr, local_repos: &HashMap<String, PathBuf>) -> ArtifactView {
    let content = read_source_content(&adr.source_uri, Some(&adr.relative_path), local_repos);
    let sidecar_warning = local_repos
        .get(adr.source_uri.trim_end_matches(".git"))
        .and_then(|repo| {
            let relative = Path::new(&adr.relative_path);
            let parent = repo.join(relative.parent()?);
            let sidecar = resolve_sidecar(&parent, relative)?;
            Some(sidecar_name_warning(&adr.relative_path, &sidecar))
        })
        .unwrap_or_default();
    let git_log = git_log_for_artifact(&adr.source_uri, Some(&adr.relative_path), local_repos);
    let latest_date = git_log.first().map_or("", |commit| commit.date.as_str());
    let (broken_refs, age_days) = artifact_staleness(
        local_repos.get(adr.source_uri.trim_end_matches(".git")),
        &adr.relative_path,
        &content.raw,
        latest_date,
    );
    ArtifactView {
        name: adr.id.clone(),
        kind: "adr".to_string(),
        module: adr.repo.clone(),
        relative_path: adr.relative_path.clone(),
        description: adr.title.clone(),
        content_preview: String::new(),
        content_body: content.body,
        raw_source: content.raw,
        metadata: content.metadata,
        providers: BTreeMap::new(),
        git_log,
        adoption: read_source_adoption(&adr.source_uri, Some(&adr.relative_path), local_repos),
        sidecar_warning,
        broken_refs,
        age_days,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
    }
}

fn discover_targets(root: &Path, providers: &[(String, String)]) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    let home = dirs::home_dir();
    if let Some(ref home_path) = home
        && has_provider_dirs(home_path, providers)
    {
        targets.push(home_path.clone());
    }
    let root_abs = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let is_home = home
        .as_ref()
        .is_some_and(|home_path| root_abs == *home_path);
    if !is_home && has_provider_dirs(&root_abs, providers) {
        targets.push(root_abs.clone());
    }
    for location in configured_locations() {
        let canonical = fs::canonicalize(&location).unwrap_or(location);
        if canonical != root_abs
            && !targets.contains(&canonical)
            && has_provider_dirs(&canonical, providers)
        {
            targets.push(canonical);
        }
    }
    targets
}

/// Additional scan locations from the `forge watch` watchlist
/// (`~/.config/forge/watchlist.yaml`).
fn configured_locations() -> Vec<PathBuf> {
    crate::cli::watchlist::watched_locations()
}

fn has_provider_dirs(base: &Path, providers: &[(String, String)]) -> bool {
    providers.iter().any(|(_, dir)| base.join(dir).is_dir())
}

pub fn discover_local_repos(root: &Path) -> HashMap<String, PathBuf> {
    let mut repos = HashMap::new();
    let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());

    let mut search_dirs: Vec<PathBuf> = Vec::new();
    if let Some(parent) = canonical.parent() {
        search_dirs.push(parent.to_path_buf());
    }
    for location in configured_locations() {
        let loc = fs::canonicalize(&location).unwrap_or(location);
        register_repo(&loc, &mut repos);
        if let Some(parent) = loc.parent() {
            search_dirs.push(parent.to_path_buf());
        }
    }

    for search_dir in search_dirs {
        let Ok(entries) = fs::read_dir(&search_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            register_repo(&entry.path(), &mut repos);
        }
    }
    repos
}

fn register_repo(path: &Path, repos: &mut HashMap<String, PathBuf>) {
    if !path.is_dir() || !path.join(".git").exists() {
        return;
    }
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output();
    if let Ok(output) = output
        && output.status.success()
    {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let normalized = url.trim_end_matches(".git").to_string();
        repos.insert(normalized, path.to_path_buf());
    }
}

fn git_remote(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if url.is_empty() {
        None
    } else {
        Some(url.trim_end_matches(".git").to_string())
    }
}

/// Scans a forge module's source artifacts (`agents/`, `rules/`, `skills/`)
/// and their adoption provenance sidecars. Returns `None` if the directory
/// holds no source artifacts.
fn scan_source_module(root: &Path) -> Option<ModuleView> {
    let source_uri = git_remote(root).unwrap_or_else(|| root.to_string_lossy().to_string());
    let module_name = root.file_name().map_or_else(
        || "module".to_string(),
        |name| name.to_string_lossy().to_string(),
    );

    let mut artifacts = Vec::new();
    artifacts.extend(scan_flat_kind(root, "agents"));
    artifacts.extend(scan_flat_kind(root, "rules"));
    artifacts.extend(scan_skill_kind(root));

    if artifacts.is_empty() {
        return None;
    }
    Some(ModuleView {
        name: module_name,
        version: String::new(),
        description: String::new(),
        source_uri,
        is_target: true,
        artifacts,
    })
}

fn scan_flat_kind(root: &Path, kind: &str) -> Vec<ArtifactView> {
    let kind_dir = root.join(kind);
    let Ok(entries) = fs::read_dir(&kind_dir) else {
        return Vec::new();
    };
    let mut artifacts = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(name) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };
        let relative_path = format!("{kind}/{name}.md");
        let sidecar = resolve_sidecar(&kind_dir, Path::new(&relative_path))
            .unwrap_or_else(|| kind_dir.join(".provenance").join(format!("{name}.yaml")));
        artifacts.push(build_source_artifact(
            root,
            kind,
            &name,
            &path,
            &relative_path,
            &sidecar,
        ));
    }
    artifacts
}

fn scan_skill_kind(root: &Path) -> Vec<ArtifactView> {
    let skills_root = root.join("skills");
    let Ok(entries) = fs::read_dir(&skills_root) else {
        return Vec::new();
    };
    let mut artifacts = Vec::new();
    for entry in entries.flatten() {
        let skill_dir = entry.path();
        if !skill_dir.is_dir() {
            continue;
        }
        let skill_file = skill_dir.join("SKILL.md");
        if !skill_file.is_file() {
            continue;
        }
        let Some(name) = skill_dir
            .file_name()
            .map(|dir| dir.to_string_lossy().to_string())
        else {
            continue;
        };
        let relative_path = format!("skills/{name}/SKILL.md");
        let sidecar = resolve_sidecar(&skill_dir, Path::new(&relative_path))
            .unwrap_or_else(|| skill_dir.join(".provenance").join(format!("{name}.yaml")));
        let mut artifact =
            build_source_artifact(root, "skills", &name, &skill_file, &relative_path, &sidecar);
        artifact.companions = read_source_companions(&skill_dir, &name);
        artifacts.push(artifact);
    }
    artifacts
}

fn build_source_artifact(
    repo_root: &Path,
    kind: &str,
    name: &str,
    file_path: &Path,
    relative_path: &str,
    sidecar_path: &Path,
) -> ArtifactView {
    let raw_source = fs::read_to_string(file_path).unwrap_or_default();
    let description = extract_frontmatter_field(&raw_source, "description");
    let metadata = parse_frontmatter(&raw_source);
    let content_body = strip_frontmatter(&raw_source);
    let content_preview = if description.is_empty() {
        content_body.lines().take(10).collect::<Vec<_>>().join("\n")
    } else {
        String::new()
    };
    let adoption = fs::read_to_string(sidecar_path)
        .ok()
        .and_then(|content| parse_adoption(&content));
    let git_log = git_log_in_repo(repo_root, relative_path);
    let sidecar_warning = sidecar_name_warning(relative_path, sidecar_path);
    ArtifactView {
        name: name.to_string(),
        kind: kind.to_string(),
        module: String::new(),
        relative_path: relative_path.to_string(),
        description,
        content_preview,
        content_body,
        raw_source,
        metadata,
        providers: BTreeMap::new(),
        git_log,
        adoption,
        sidecar_warning,
        broken_refs: Vec::new(),
        age_days: None,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
    }
}

/// Discovers harness and model qualifier overrides of a base artifact in the
/// source tree (PROV-0005): `<kind-dir>/<provider>/<file>` for harness-level and
/// `<kind-dir>/<provider>/<model>/<file>` for model-level, plus the `user/`
/// overlay. The base directory is the artifact file's parent, so the same logic
/// serves flat kinds (rules, agents) and skill directories alike.
fn collect_variants(repo: &Path, relative_path: &str, provider_names: &[String]) -> Vec<Variant> {
    let base_file = repo.join(relative_path);
    let Some(base_dir) = base_file.parent() else {
        return Vec::new();
    };
    let Some(file_name) = base_file.file_name().and_then(|name| name.to_str()) else {
        return Vec::new();
    };
    let mut qualifiers: Vec<&str> = provider_names.iter().map(String::as_str).collect();
    qualifiers.push("user");
    let mut variants = Vec::new();
    for provider in qualifiers {
        let provider_dir = base_dir.join(provider);
        if !provider_dir.is_dir() {
            continue;
        }
        let provider_file = provider_dir.join(file_name);
        if provider_file.is_file() {
            variants.push(make_variant(repo, provider, "", &provider_file));
        }
        let Ok(entries) = fs::read_dir(&provider_dir) else {
            continue;
        };
        let mut model_dirs: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().is_dir())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .collect();
        model_dirs.sort();
        for model in model_dirs {
            let model_file = provider_dir.join(&model).join(file_name);
            if model_file.is_file() {
                variants.push(make_variant(repo, provider, &model, &model_file));
            }
        }
    }
    variants
}

fn make_variant(repo: &Path, provider: &str, model: &str, file: &Path) -> Variant {
    let relative_path = file
        .strip_prefix(repo)
        .unwrap_or(file)
        .to_string_lossy()
        .to_string();
    let content = fs::read_to_string(file).unwrap_or_default();
    let mode = match extract_frontmatter_field(&content, "mode") {
        mode if mode.is_empty() => "replace".to_string(),
        mode => mode,
    };
    let qualifier = if model.is_empty() {
        provider.to_string()
    } else {
        format!("{provider}/{model}")
    };
    Variant {
        qualifier,
        provider: provider.to_string(),
        model: model.to_string(),
        relative_path,
        mode,
    }
}

/// Returns a warning when the resolved sidecar uses a non-canonical filename.
/// Canonical is `{file_stem}.yaml` (e.g. `SKILL.yaml` for a skill's `SKILL.md`).
/// Empty when the sidecar is canonical or absent.
fn sidecar_name_warning(relative_path: &str, sidecar_path: &Path) -> String {
    if !sidecar_path.is_file() {
        return String::new();
    }
    let Some(actual) = sidecar_path.file_name().and_then(|name| name.to_str()) else {
        return String::new();
    };
    let stem = Path::new(relative_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default();
    let canonical = format!("{stem}.yaml");
    if actual == canonical {
        String::new()
    } else {
        format!("non-canonical sidecar name '{actual}' (canonical is '{canonical}')")
    }
}

/// Reads companion `.md` files in a source skill directory (everything
/// except `SKILL.md`), to fold under the parent skill.
fn read_source_companions(skill_dir: &Path, skill_name: &str) -> Vec<Companion> {
    let Ok(entries) = fs::read_dir(skill_dir) else {
        return Vec::new();
    };
    let mut companions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };
        if stem == "SKILL" {
            continue;
        }
        let raw_source = fs::read_to_string(&path).unwrap_or_default();
        companions.push(Companion {
            description: extract_frontmatter_field(&raw_source, "description"),
            content_body: strip_frontmatter(&raw_source),
            relative_path: format!("skills/{skill_name}/{stem}.md"),
            name: stem,
            raw_source,
        });
    }
    companions.sort_by(|a, b| a.name.cmp(&b.name));
    companions
}

fn git_log_in_repo(repo: &Path, file_rel: &str) -> Vec<GitCommit> {
    let output = Command::new("git")
        .args(["log", "--follow", "-n", "5", GIT_LOG_FORMAT, "--", file_rel])
        .current_dir(repo)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut commits = parse_git_log(&String::from_utf8_lossy(&output.stdout));
    enrich_commits_with_entire(repo, &mut commits);
    commits
}

struct PendingCompanion {
    source_uri: String,
    parent: String,
    companion: Companion,
}

fn scan_target(
    target_base: &Path,
    modules: &mut BTreeMap<String, ModuleView>,
    summary: &mut StatusSummary,
    local_repos: &HashMap<String, PathBuf>,
    pending_companions: &mut Vec<PendingCompanion>,
    providers: &[(String, String)],
) {
    for (provider_name, provider_dir) in providers {
        let provider_path = target_base.join(provider_dir);
        if !provider_path.is_dir() {
            continue;
        }
        let entries = load_manifest(&provider_path);
        for (relative_key, entry) in &entries {
            let Some((kind, deployed_name)) = parse_artifact_key(relative_key) else {
                continue;
            };
            let source = resolve_source(&provider_path, relative_key, entry);

            if let Some(pending) = companion_entry(&provider_path, relative_key, &source) {
                pending_companions.push(pending);
                continue;
            }

            let canonical_name =
                resolve_source_name(&provider_path, entry).unwrap_or(deployed_name);
            let source_path = resolve_source_path(&provider_path, entry);
            let status = deployed_status(
                &provider_path,
                relative_key,
                entry,
                &source,
                source_path.as_deref(),
                local_repos,
            );
            tally_status(summary, status);

            let module_view = modules.entry(source.clone()).or_insert_with(|| ModuleView {
                name: module_name_from_source(&source),
                version: String::new(),
                description: String::new(),
                source_uri: source,
                is_target: false,
                artifacts: Vec::new(),
            });

            let provider_status = ProviderStatus {
                status,
                fingerprint: Some(entry.fingerprint.clone()),
            };
            let existing = module_view
                .artifacts
                .iter_mut()
                .find(|artifact| artifact.name == canonical_name && artifact.kind == kind);
            if let Some(artifact) = existing {
                artifact
                    .providers
                    .insert(provider_name.clone(), provider_status);
            } else {
                let artifact = build_deployed_artifact(
                    &provider_path,
                    relative_key,
                    kind,
                    canonical_name,
                    &module_view.source_uri,
                    source_path.as_deref(),
                    (provider_name.as_str(), provider_status),
                    local_repos,
                );
                module_view.artifacts.push(artifact);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_deployed_artifact(
    provider_path: &Path,
    relative_key: &str,
    kind: &str,
    canonical_name: String,
    source_uri: &str,
    source_path: Option<&str>,
    provider: (&str, ProviderStatus),
    local_repos: &HashMap<String, PathBuf>,
) -> ArtifactView {
    let mut providers = BTreeMap::new();
    providers.insert(provider.0.to_string(), provider.1);
    let source_content = read_source_content(source_uri, source_path, local_repos);
    let deployed_content = read_artifact_content(provider_path, relative_key);
    let description = if source_content.description.is_empty() {
        deployed_content.description
    } else {
        source_content.description
    };
    // Prefer the source body: it keeps reference-link definitions (assembly
    // strips them), so the markdown preview resolves reflinks.
    let content_body = if source_content.body.is_empty() {
        deployed_content.body
    } else {
        source_content.body
    };
    let content_preview = if description.is_empty() {
        content_body.lines().take(10).collect::<Vec<_>>().join("\n")
    } else {
        String::new()
    };
    ArtifactView {
        name: canonical_name,
        kind: kind.to_string(),
        module: String::new(),
        relative_path: relative_key.to_string(),
        description,
        content_preview,
        content_body,
        raw_source: source_content.raw,
        metadata: source_content.metadata,
        providers,
        git_log: git_log_for_artifact(source_uri, source_path, local_repos),
        adoption: read_source_adoption(source_uri, source_path, local_repos),
        sidecar_warning: String::new(),
        broken_refs: Vec::new(),
        age_days: None,
        module_tint: 0,
        companions: Vec::new(),
        variants: Vec::new(),
    }
}

/// Builds a `PendingCompanion` from a deployed companion manifest entry,
/// or `None` if the entry is not a skill companion file.
fn companion_entry(
    provider_path: &Path,
    relative_key: &str,
    source_uri: &str,
) -> Option<PendingCompanion> {
    let (parent, companion_name) = companion_of(relative_key)?;
    let content = read_artifact_content(provider_path, relative_key);
    let raw_source = fs::read_to_string(provider_path.join(relative_key)).unwrap_or_default();
    Some(PendingCompanion {
        source_uri: source_uri.to_string(),
        parent,
        companion: Companion {
            name: companion_name,
            relative_path: relative_key.to_string(),
            description: content.description,
            content_body: content.body,
            raw_source,
        },
    })
}

/// Detects a skill companion file: `skills/<Parent>/<Name>.md` where
/// `<Name>` is not `SKILL`. Returns `(parent, companion_name)`.
fn companion_of(relative_key: &str) -> Option<(String, String)> {
    let segments: Vec<&str> = relative_key.split('/').collect();
    if segments.len() != 3 || segments[0] != "skills" {
        return None;
    }
    let stem = segments[2]
        .trim_end_matches(".md")
        .trim_end_matches(".toml");
    if stem == "SKILL" {
        return None;
    }
    Some((segments[1].to_string(), stem.to_string()))
}

/// Attaches collected companion files to their parent skill artifacts,
/// deduplicating across providers by companion name.
fn attach_companions(modules: &mut BTreeMap<String, ModuleView>, pending: Vec<PendingCompanion>) {
    for item in pending {
        let Some(module) = modules.get_mut(&item.source_uri) else {
            continue;
        };
        let Some(parent) = module
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == "skills" && artifact.name == item.parent)
        else {
            continue;
        };
        if parent
            .companions
            .iter()
            .any(|existing| existing.name == item.companion.name)
        {
            continue;
        }
        parent.companions.push(item.companion);
    }
    for module in modules.values_mut() {
        for artifact in &mut module.artifacts {
            artifact.companions.sort_by(|a, b| a.name.cmp(&b.name));
        }
    }
}

/// Reads the source-repo adoption sidecar for a deployed artifact.
/// `skills/X/SKILL.md` -> `skills/X/.provenance/SKILL.yaml`,
/// `agents/X.md` -> `agents/.provenance/X.yaml`.
pub fn read_source_adoption(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> Option<Adoption> {
    parse_adoption(&read_source_sidecar(source_uri, source_path, local_repos)?)
}

/// Resolves a source artifact's provenance sidecar. The canonical name is
/// file-keyed (`SKILL.yaml` for `SKILL.md`, `<Companion>.yaml` for a companion)
/// so multiple files sharing one `.provenance` directory stay distinct. Older
/// copied modules used non-canonical names (`<file>.md.yaml`, or `<SkillName>.yaml`
/// keyed on the directory) which are tolerated as fallbacks for display.
fn resolve_sidecar(parent_dir: &Path, source_path: &Path) -> Option<PathBuf> {
    let provenance = parent_dir.join(".provenance");
    let file_name = source_path.file_name()?.to_string_lossy().to_string();
    let file_stem = source_path.file_stem()?.to_string_lossy().to_string();
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.push(provenance.join(format!("{file_stem}.yaml")));
    candidates.push(provenance.join(format!("{file_name}.yaml")));
    if file_name == "SKILL.md"
        && let Some(dir_name) = source_path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_string())
    {
        candidates.push(provenance.join(format!("{dir_name}.yaml")));
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

/// Returns the raw provenance sidecar YAML for a source artifact, or `None`.
pub fn read_source_sidecar(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> Option<String> {
    let normalized = source_uri.trim_end_matches(".git");
    let repo_path = local_repos.get(normalized)?;
    let file_rel = Path::new(source_path?);
    let parent_dir = repo_path.join(file_rel.parent()?);
    let sidecar = resolve_sidecar(&parent_dir, file_rel)?;
    fs::read_to_string(&sidecar).ok()
}

struct SourceContent {
    description: String,
    body: String,
    raw: String,
    metadata: Vec<(String, String)>,
}

fn read_source_content(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> SourceContent {
    let empty = SourceContent {
        description: String::new(),
        body: String::new(),
        raw: String::new(),
        metadata: Vec::new(),
    };
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo_path) = local_repos.get(normalized) else {
        return empty;
    };
    let Some(file_rel) = source_path else {
        return empty;
    };
    let file_path = repo_path.join(file_rel);
    let Ok(content) = fs::read_to_string(&file_path) else {
        return empty;
    };
    let description = extract_frontmatter_field(&content, "description");
    let metadata = parse_frontmatter(&content);
    let body = strip_frontmatter(&content);
    SourceContent {
        description,
        body,
        raw: content,
        metadata,
    }
}

/// Parses flat frontmatter fields, preserving their source order for display.
fn parse_frontmatter(content: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let Some(rest) = content.strip_prefix("---") else {
        return fields;
    };
    let Some(end) = rest.find("\n---") else {
        return fields;
    };
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !key.is_empty() && !value.is_empty() {
                fields.push((key, value));
            }
        }
    }
    fields
}

#[derive(serde::Deserialize)]
struct Sidecar {
    provenance: Statement,
}
#[derive(serde::Deserialize)]
struct Statement {
    #[serde(default)]
    subject: Vec<SubjectRef>,
    predicate: Predicate,
    #[serde(default)]
    attribution: Attribution,
}
#[derive(serde::Deserialize, Default)]
struct SubjectRef {
    #[serde(default)]
    digest: DependencyDigest,
}
#[derive(serde::Deserialize)]
struct Predicate {
    #[serde(rename = "buildDefinition")]
    build_definition: BuildDefinition,
}
#[derive(serde::Deserialize)]
struct BuildDefinition {
    #[serde(rename = "buildType", default)]
    build_type: String,
    #[serde(rename = "externalParameters", default)]
    external_parameters: ExternalParameters,
    #[serde(rename = "resolvedDependencies", default)]
    resolved_dependencies: Vec<ResolvedDependency>,
}
#[derive(serde::Deserialize, Default)]
struct ExternalParameters {
    #[serde(default)]
    source: String,
    #[serde(default)]
    upstream_url: String,
    #[serde(default)]
    upstream_commit: String,
    #[serde(default)]
    transforms_applied: Vec<String>,
}
#[derive(serde::Deserialize)]
struct ResolvedDependency {
    #[serde(default)]
    name: String,
    #[serde(default)]
    uri: String,
    #[serde(default)]
    digest: DependencyDigest,
}
#[derive(serde::Deserialize, Default)]
struct DependencyDigest {
    #[serde(default)]
    sha256: String,
}
#[derive(serde::Deserialize, Default)]
struct Attribution {
    #[serde(default)]
    upstream_author: String,
    #[serde(default)]
    upstream_license: String,
    #[serde(default)]
    adopted_by: String,
}

/// Parses an `adopt/v1` or `copy/v1` provenance sidecar into a view `Adoption`.
fn parse_adoption(content: &str) -> Option<Adoption> {
    let sidecar: Sidecar = serde_yaml::from_str(content).ok()?;
    let definition = &sidecar.provenance.predicate.build_definition;
    let params = &definition.external_parameters;
    let kind = if definition.build_type.contains("adopt") {
        "adopt"
    } else if definition.build_type.contains("copy") {
        "copy"
    } else {
        "build"
    };
    let source = if params.upstream_url.is_empty() {
        params.source.clone()
    } else {
        params.upstream_url.clone()
    };
    let source_sha = definition
        .resolved_dependencies
        .iter()
        .find(|dependency| dependency.name == "upstream")
        .map(|dependency| dependency.digest.sha256.clone())
        .unwrap_or_default();
    let dependencies = definition
        .resolved_dependencies
        .iter()
        .filter(|dependency| dependency.name != "upstream" && !dependency.name.is_empty())
        .map(|dependency| Dependency {
            name: dependency.name.clone(),
            uri: dependency.uri.clone(),
            sha: dependency.digest.sha256.clone(),
        })
        .collect();
    let (source_repo, source_label) = shorten_source(&source);
    let attribution = &sidecar.provenance.attribution;
    Some(Adoption {
        kind: kind.to_string(),
        source,
        source_repo,
        source_label,
        source_sha,
        commit: params.upstream_commit.clone(),
        transforms: params.transforms_applied.clone(),
        author: attribution.upstream_author.clone(),
        license: attribution.upstream_license.clone(),
        adopted_by: attribution.adopted_by.clone(),
        dependencies,
    })
}

/// Shortens a source URL to `(repo_url, "owner/repo")`. For a GitHub/GitLab
/// blob URL like `https://github.com/owner/repo/blob/SHA/path`, returns the
/// repo root and `owner/repo` label. Non-URL sources return `(source, source)`.
fn shorten_source(source: &str) -> (String, String) {
    let Some(rest) = source
        .strip_prefix("https://")
        .or_else(|| source.strip_prefix("http://"))
    else {
        return (source.to_string(), source.to_string());
    };
    let segments: Vec<&str> = rest.split('/').collect();
    if segments.len() < 3 {
        return (source.to_string(), source.to_string());
    }
    let host = segments[0];
    let owner = segments[1];
    let repo = segments[2];
    let repo_url = format!("https://{host}/{owner}/{repo}");
    let label = format!("{owner}/{repo}");
    (repo_url, label)
}

struct ArtifactContent {
    description: String,
    body: String,
}

fn read_artifact_content(provider_path: &Path, relative_key: &str) -> ArtifactContent {
    let file_path = provider_path.join(relative_key);
    let Ok(content) = fs::read_to_string(&file_path) else {
        return ArtifactContent {
            description: String::new(),
            body: String::new(),
        };
    };
    let description = extract_frontmatter_field(&content, "description");
    let body = strip_frontmatter(&content);
    ArtifactContent { description, body }
}

pub fn extract_frontmatter_field(content: &str, field: &str) -> String {
    let Some(rest) = content.strip_prefix("---") else {
        return String::new();
    };
    let Some(end) = rest.find("\n---") else {
        return String::new();
    };
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix(&format!("{field}:")) {
            return value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
        }
    }
    String::new()
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

fn resolve_source(provider_path: &Path, relative_key: &str, entry: &ManifestEntry) -> String {
    if let Some(ref provenance_rel) = entry.provenance {
        let sidecar_path = provider_path.join(provenance_rel);
        if let Ok(content) = fs::read_to_string(&sidecar_path)
            && let Some(source_uri) = extract_source_uri(&content)
        {
            return source_uri;
        }
    }
    let target_label = provider_path
        .parent()
        .and_then(|parent| parent.file_name())
        .map_or_else(
            || "unknown".to_string(),
            |name| name.to_string_lossy().to_string(),
        );
    format!(
        "{target_label}/{}",
        relative_key.split('/').next().unwrap_or("unknown")
    )
}

fn resolve_source_name(provider_path: &Path, entry: &ManifestEntry) -> Option<String> {
    let provenance_rel = entry.provenance.as_ref()?;
    let sidecar_path = provider_path.join(provenance_rel);
    let content = fs::read_to_string(&sidecar_path).ok()?;
    extract_dependency_uri(&content)
}

fn resolve_source_path(provider_path: &Path, entry: &ManifestEntry) -> Option<String> {
    let provenance_rel = entry.provenance.as_ref()?;
    let sidecar_path = provider_path.join(provenance_rel);
    let content = fs::read_to_string(&sidecar_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        if let Some(uri) = trimmed.strip_prefix("uri:") {
            return Some(uri.trim().to_string());
        }
    }
    None
}

fn extract_dependency_uri(sidecar_content: &str) -> Option<String> {
    for line in sidecar_content.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        if let Some(uri) = trimmed.strip_prefix("uri:") {
            let path = uri.trim();
            let segments: Vec<&str> = path.split('/').collect();
            let filename = segments.last().unwrap_or(&path);
            let stem = filename.trim_end_matches(".md").trim_end_matches(".toml");
            if stem == "SKILL" && segments.len() >= 3 {
                return Some(segments[segments.len() - 2].to_string());
            }
            return Some(stem.to_string());
        }
    }
    None
}

fn extract_source_uri(sidecar_content: &str) -> Option<String> {
    for line in sidecar_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("source:") {
            return Some(trimmed.trim_start_matches("source:").trim().to_string());
        }
    }
    None
}

fn module_name_from_source(source_uri: &str) -> String {
    source_uri
        .rsplit('/')
        .next()
        .unwrap_or(source_uri)
        .trim_end_matches(".git")
        .to_string()
}

fn load_manifest(target_dir: &Path) -> HashMap<String, ManifestEntry> {
    let manifest_path = target_dir.join(".manifest");
    let Ok(content) = fs::read_to_string(&manifest_path) else {
        return HashMap::new();
    };
    manifest::read(&content).unwrap_or_default()
}

fn parse_artifact_key(key: &str) -> Option<(&str, String)> {
    let parts: Vec<&str> = key.splitn(3, '/').collect();
    if parts.len() < 2 {
        return None;
    }
    let kind = match parts[0] {
        "skills" | "agents" | "rules" => parts[0],
        _ => return None,
    };
    let name = parts[1].trim_end_matches(".md").trim_end_matches(".toml");
    Some((kind, name.to_string()))
}

/// Deploy status with precedence: a deployed file edited since deploy is
/// `Modified`; an unchanged file whose source drifted is `Stale`.
fn deployed_status(
    provider_path: &Path,
    relative_key: &str,
    entry: &ManifestEntry,
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> FileStatus {
    let status = compute_deployed_status(provider_path, relative_key, entry);
    if status == FileStatus::Unchanged
        && is_stale(provider_path, entry, source_uri, source_path, local_repos)
    {
        return FileStatus::Stale;
    }
    status
}

fn compute_deployed_status(
    target_dir: &Path,
    relative_key: &str,
    entry: &ManifestEntry,
) -> FileStatus {
    let target_path = target_dir.join(relative_key);
    let Ok(content) = fs::read_to_string(&target_path) else {
        return FileStatus::New;
    };
    let current_sha = manifest::content_sha256(&content);
    if current_sha == entry.fingerprint {
        FileStatus::Unchanged
    } else {
        FileStatus::Modified
    }
}

fn tally_status(summary: &mut StatusSummary, status: FileStatus) {
    match status {
        FileStatus::Unchanged => summary.unchanged += 1,
        FileStatus::Stale => summary.stale += 1,
        FileStatus::Modified => summary.modified += 1,
        FileStatus::New => summary.new += 1,
    }
}

/// A deployed artifact is stale when its source changed since deploy: the
/// current source file SHA differs from the input SHA recorded in the
/// deployed `assemble/v1` sidecar's `resolvedDependencies`.
fn is_stale(
    provider_path: &Path,
    entry: &ManifestEntry,
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> bool {
    let Some(provenance_rel) = entry.provenance.as_ref() else {
        return false;
    };
    let Ok(sidecar) = fs::read_to_string(provider_path.join(provenance_rel)) else {
        return false;
    };
    let Some(recorded_sha) = recorded_input_sha(&sidecar) else {
        return false;
    };
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo) = local_repos.get(normalized) else {
        return false;
    };
    let Some(rel) = source_path else {
        return false;
    };
    let Ok(current) = fs::read_to_string(repo.join(rel)) else {
        return false;
    };
    manifest::content_sha256(&current) != recorded_sha
}

/// Reads the first `resolvedDependencies` digest from an `assemble/v1`
/// sidecar (the source input SHA captured at deploy time).
fn recorded_input_sha(sidecar_content: &str) -> Option<String> {
    let sidecar: Sidecar = serde_yaml::from_str(sidecar_content).ok()?;
    sidecar
        .provenance
        .predicate
        .build_definition
        .resolved_dependencies
        .first()
        .map(|dependency| dependency.digest.sha256.clone())
        .filter(|sha| !sha.is_empty())
}

/// Reads the subject digest from a sidecar (the artifact's own content hash at
/// copy time), used to detect a copy edited after adoption.
fn recorded_subject_sha(sidecar_content: &str) -> Option<String> {
    let sidecar: Sidecar = serde_yaml::from_str(sidecar_content).ok()?;
    sidecar
        .provenance
        .subject
        .first()
        .map(|subject| subject.digest.sha256.clone())
        .filter(|sha| !sha.is_empty())
}

/// Source file content at the commit that was deployed, found by matching the
/// deployed sidecar's recorded input hash (`recorded_sha`) against the source
/// file's recent git history. Returns `None` when the current source already
/// matches (no drift) or the deploy commit is not in recent history.
pub fn source_at_deploy(
    recorded_sha: &str,
    source_uri: &str,
    source_path: &str,
    local_repos: &HashMap<String, PathBuf>,
) -> Option<String> {
    let repo = local_repos.get(source_uri.trim_end_matches(".git"))?;
    let current = fs::read_to_string(repo.join(source_path)).ok()?;
    if manifest::content_sha256(&current) == recorded_sha {
        return None;
    }
    // Bounded history scan: if the deploy commit is older than this window the
    // drift diff is silently unavailable (the artifact still shows as stale via
    // its provenance). A richer "deploy predates history" signal is a follow-up.
    for sha in recent_commit_shas(repo, source_path, 200) {
        if let Some(content) = git_show_file(repo, &sha, source_path)
            && manifest::content_sha256(&content) == recorded_sha
        {
            return Some(content);
        }
    }
    None
}

/// Recent commit SHAs touching a file, newest first.
fn recent_commit_shas(repo: &Path, file_rel: &str, limit: usize) -> Vec<String> {
    let output = Command::new("git")
        .args([
            "log",
            "--follow",
            "-n",
            &limit.to_string(),
            "--format=%H",
            "--",
            file_rel,
        ])
        .current_dir(repo)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// File content at a specific commit (`git show {sha}:{path}`).
fn git_show_file(repo: &Path, sha: &str, path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["show", &format!("{sha}:{path}")])
        .current_dir(repo)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn git_log_for_artifact(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> Vec<GitCommit> {
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo_path) = local_repos.get(normalized) else {
        return Vec::new();
    };
    let Some(file_path) = source_path else {
        return Vec::new();
    };
    let output = Command::new("git")
        .args([
            "log",
            "--follow",
            "-n",
            "5",
            GIT_LOG_FORMAT,
            "--",
            file_path,
        ])
        .current_dir(repo_path)
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut commits = parse_git_log(&String::from_utf8_lossy(&output.stdout));
    enrich_commits_with_entire(repo_path, &mut commits);
    commits
}

/// One NUL-delimited record per commit: sha, subject, author-date, author,
/// `Entire-Checkpoint` trailer. NUL fields survive subjects containing any
/// printable character; records are newline-separated (every field is
/// single-line).
const GIT_LOG_FORMAT: &str =
    "--format=%H%x00%s%x00%ai%x00%an%x00%(trailers:key=Entire-Checkpoint,valueonly,separator=%x20)";

fn parse_git_log(raw: &str) -> Vec<GitCommit> {
    raw.lines()
        .filter(|line| !line.is_empty())
        .filter_map(|line| {
            let mut fields = line.split('\u{0}');
            let sha = fields.next()?.to_string();
            if sha.is_empty() {
                return None;
            }
            Some(GitCommit {
                message: fields.next().unwrap_or_default().to_string(),
                date: fields.next().unwrap_or_default().to_string(),
                author: fields.next().unwrap_or_default().to_string(),
                checkpoint: fields.next().unwrap_or_default().trim().to_string(),
                sha,
                ..GitCommit::default()
            })
        })
        .collect()
}

/// Fills the agent-intent facets (`prompt`, `session_count`) for every commit
/// that carries an `Entire-Checkpoint` trailer, reading the checkpoint's
/// sessions from the committed `entire/checkpoints/v1` branch. Commits without
/// a checkpoint, or repos without the branch, are left untouched.
fn enrich_commits_with_entire(repo: &Path, commits: &mut [GitCommit]) {
    for commit in commits.iter_mut() {
        if commit.checkpoint.len() < 3 {
            continue;
        }
        let (shard, rest) = commit.checkpoint.split_at(2);
        let base = format!("entire/checkpoints/v1:{shard}/{rest}");
        let mut sessions: Vec<usize> = git_show_lines(repo, &format!("{base}/"))
            .into_iter()
            .filter_map(|name| name.trim_end_matches('/').parse::<usize>().ok())
            .collect();
        sessions.sort_unstable();
        commit.session_count = sessions.len();
        commit.prompt = checkpoint_prompt(repo, &base, &sessions);
    }
}

/// Picks a one-line intent teaser from a checkpoint's sessions: the first
/// session prompt that is not a compaction-continuation summary, falling back
/// to the first session's opening line.
fn checkpoint_prompt(repo: &Path, base: &str, sessions: &[usize]) -> String {
    let mut fallback = String::new();
    for index in sessions {
        let prompt = git_show(repo, &format!("{base}/{index}/prompt.txt")).unwrap_or_default();
        let first_line = prompt
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("");
        if first_line.is_empty() {
            continue;
        }
        if fallback.is_empty() {
            fallback = first_line.to_string();
        }
        if !first_line.starts_with("This session is being continued") {
            return truncate_prompt(first_line);
        }
    }
    truncate_prompt(&fallback)
}

fn truncate_prompt(line: &str) -> String {
    const LIMIT: usize = 110;
    if line.chars().count() <= LIMIT {
        return line.to_string();
    }
    let cut: String = line.chars().take(LIMIT).collect();
    format!("{}\u{2026}", cut.trim_end())
}

/// Runs `git show <object>` in a repo, returning its stdout or `None` on any
/// failure (missing branch, missing path, non-utf8).
fn git_show(repo: &Path, object: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["show", object])
        .current_dir(repo)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Lists the entry names directly under a tree object (`git show <tree>/`).
fn git_show_lines(repo: &Path, tree: &str) -> Vec<String> {
    git_show(repo, tree)
        .map(|text| {
            text.lines()
                .skip_while(|line| !line.trim().is_empty())
                .filter(|line| !line.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn collect_provenance(
    target_base: &Path,
    providers: &[(String, String)],
    provenance: &mut Vec<ProvenanceView>,
) {
    let target_label = deployment_target_label(target_base);
    let mut by_source: BTreeMap<String, ProvenanceView> = BTreeMap::new();
    for (harness_name, provider_dir) in providers {
        let provider_path = target_base.join(provider_dir);
        if !provider_path.is_dir() {
            continue;
        }
        walk_provenance_dirs(&provider_path, harness_name, &target_label, &mut by_source);
    }
    provenance.extend(by_source.into_values());
}

/// Short label for a deployment target base: `~` for the home directory,
/// otherwise the final path component.
fn deployment_target_label(target_base: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && target_base == home
    {
        return "~".to_string();
    }
    target_base.file_name().map_or_else(
        || target_base.to_string_lossy().to_string(),
        |name| name.to_string_lossy().to_string(),
    )
}

fn walk_provenance_dirs(
    provider_path: &Path,
    harness_name: &str,
    target_label: &str,
    by_source: &mut BTreeMap<String, ProvenanceView>,
) {
    for content_dir in content_kinds() {
        let kind_dir = provider_path.join(content_dir);
        let prov_dirs = find_provenance_dirs(&kind_dir);
        for prov_dir in &prov_dirs {
            let Ok(entries) = fs::read_dir(prov_dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|ext| ext.to_str()) != Some("yaml") {
                    continue;
                }
                let Ok(sidecar_content) = fs::read_to_string(&path) else {
                    continue;
                };
                let source =
                    extract_source_uri(&sidecar_content).unwrap_or_else(|| "unknown".to_string());
                let parsed =
                    parse_sidecar(&sidecar_content, provider_path, harness_name, target_label);
                let record = by_source
                    .entry(source.clone())
                    .or_insert_with(|| ProvenanceView {
                        source_uri: source,
                        verified: 0,
                        total: 0,
                        orphans: Vec::new(),
                        artifacts: Vec::new(),
                    });
                record.total += 1;
                if parsed.verified {
                    record.verified += 1;
                }
                record.artifacts.push(parsed);
            }
        }
    }
}

fn find_provenance_dirs(base: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let direct = base.join(".provenance");
    if direct.is_dir() {
        dirs.push(direct);
    }
    let Ok(entries) = fs::read_dir(base) else {
        return dirs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let nested = path.join(".provenance");
            if nested.is_dir() {
                dirs.push(nested);
            }
        }
    }
    dirs
}

fn parse_sidecar(
    sidecar_content: &str,
    provider_path: &Path,
    harness_name: &str,
    target_label: &str,
) -> ProvenanceArtifact {
    let mut subject_name = String::new();
    let mut expected_sha = String::new();
    let mut source_path = String::new();
    for line in sidecar_content.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        if let Some(name) = trimmed.strip_prefix("name:") {
            subject_name = name.trim().to_string();
        }
        if let Some(sha) = trimmed.strip_prefix("sha256:")
            && expected_sha.is_empty()
        {
            expected_sha = sha.trim().to_string();
        }
        if let Some(uri) = trimmed.strip_prefix("uri:") {
            source_path = uri.trim().to_string();
        }
    }
    let deployed_rel = subject_name
        .split('/')
        .skip(1)
        .collect::<Vec<_>>()
        .join("/");
    let deployed_path = provider_path.join(&deployed_rel);
    let deployed_sha = fs::read_to_string(&deployed_path)
        .map(|content| manifest::content_sha256(&content))
        .unwrap_or_default();
    let verified = !expected_sha.is_empty() && deployed_sha == expected_sha;
    let input_sha = recorded_input_sha(sidecar_content).unwrap_or_default();
    ProvenanceArtifact {
        deployed_path: deployed_rel,
        source_path,
        harness: harness_name.to_string(),
        target: target_label.to_string(),
        verified,
        deployed_sha,
        expected_sha,
        input_sha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn parse_git_log_captures_checkpoint_trailer() {
        let raw = "abc123\u{0}feat: add thing\u{0}2026-06-12 10:21:46 +0200\u{0}Alice Example\u{0}933ba0519d0a\n\
                   def456\u{0}fix: tidy up\u{0}2026-06-03 01:47:07 +0200\u{0}Alice Example\u{0}\n";
        let commits = parse_git_log(raw);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].sha, "abc123");
        assert_eq!(commits[0].message, "feat: add thing");
        assert_eq!(commits[0].author, "Alice Example");
        assert_eq!(commits[0].checkpoint, "933ba0519d0a");
        assert!(commits[1].checkpoint.is_empty());
    }

    #[test]
    fn parse_git_log_skips_blank_and_empty_sha_lines() {
        let raw = "\nabc123\u{0}subject\u{0}date\u{0}author\u{0}\n\u{0}orphan\u{0}\u{0}\u{0}\n";
        let commits = parse_git_log(raw);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].sha, "abc123");
    }

    #[test]
    fn truncate_prompt_caps_long_lines_with_ellipsis() {
        let short = "tighten the sign gutter";
        assert_eq!(truncate_prompt(short), short);
        let long = "x".repeat(200);
        let capped = truncate_prompt(&long);
        assert!(capped.ends_with('\u{2026}'));
        assert!(capped.chars().count() <= 111);
    }

    #[test]
    fn resolve_sidecar_prefers_canonical_skill_yaml() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skills/Foo");
        write(&skill_dir.join(".provenance/SKILL.yaml"), "a: 1\n");
        write(&skill_dir.join(".provenance/Foo.yaml"), "a: 2\n");
        let resolved = resolve_sidecar(&skill_dir, Path::new("skills/Foo/SKILL.md")).unwrap();
        assert!(resolved.ends_with("SKILL.yaml"));
    }

    #[test]
    fn resolve_sidecar_falls_back_to_legacy_dir_name() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skills/LearnFrom");
        write(&skill_dir.join(".provenance/LearnFrom.yaml"), "a: 1\n");
        let resolved = resolve_sidecar(&skill_dir, Path::new("skills/LearnFrom/SKILL.md")).unwrap();
        assert!(resolved.ends_with("LearnFrom.yaml"));
    }

    #[test]
    fn resolve_sidecar_flat_artifact_uses_stem() {
        let temp = TempDir::new().unwrap();
        let kind_dir = temp.path().join("rules");
        write(&kind_dir.join(".provenance/Bar.yaml"), "a: 1\n");
        let resolved = resolve_sidecar(&kind_dir, Path::new("rules/Bar.md")).unwrap();
        assert!(resolved.ends_with("Bar.yaml"));
    }

    #[test]
    fn resolve_sidecar_none_when_absent() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skills/Empty");
        std::fs::create_dir_all(skill_dir.join(".provenance")).unwrap();
        assert!(resolve_sidecar(&skill_dir, Path::new("skills/Empty/SKILL.md")).is_none());
    }

    #[test]
    fn adr_state_classifies_authored_copied_modified() {
        assert_eq!(adr_state(None, "anything").0, "authored");
        let content = "decision body\n";
        let sha = manifest::content_sha256(content);
        let sidecar = format!(
            "provenance:\n    subject:\n        - digest:\n              sha256: {sha}\n    predicate:\n        buildDefinition:\n            buildType: https://github.com/N4M3Z/forge-cli/copy/v1\n            externalParameters:\n                source: https://example.com/upstream\n"
        );
        assert_eq!(adr_state(Some(&sidecar), content).0, "copied");
        assert_eq!(adr_state(Some(&sidecar), "edited body\n").0, "modified");
    }
}
