use clap::ValueEnum;
use commands::parse;
use commands::provider::ContentKind;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub(crate) enum KindFilter {
    Skills,
    Agents,
    Rules,
}

impl KindFilter {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::Agents => "agents",
            Self::Rules => "rules",
        }
    }
}

#[derive(Debug, Serialize)]
pub struct FindResult {
    name: String,
    kind: String,
    module: String,
    path: String,
    description: String,
    score: f64,
    source: String,
}

struct Candidate {
    name: String,
    kind: ContentKind,
    module: String,
    path: PathBuf,
    description: String,
    triggers: String,
}

pub fn execute(query: &str, kind: Option<KindFilter>, json: bool) -> Result<i32, String> {
    let cwd = std::env::current_dir().map_err(|error| format!("cannot read cwd: {error}"))?;
    let watched_locations = super::watchlist::watched_locations();
    let modules = discover_modules(&cwd, &watched_locations);
    let results = search_modules(&modules, query, kind);
    if json {
        print_json(&results)?;
    } else {
        print_console(&results);
    }
    Ok(0)
}

fn discover_modules(root: &Path, watched_locations: &[PathBuf]) -> Vec<PathBuf> {
    let mut modules = Vec::new();
    let mut seen = HashSet::new();
    push_module(root, &mut modules, &mut seen);

    let local_repos = commands::services::discover_local_repos(root, watched_locations);
    for repo in local_repos.values() {
        push_module(repo, &mut modules, &mut seen);
    }
    for location in watched_locations {
        push_module(location, &mut modules, &mut seen);
    }
    modules
}

fn push_module(path: &Path, modules: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
    let Ok(canonical) = fs::canonicalize(path) else {
        return;
    };
    if !canonical.join("module.yaml").is_file() || !seen.insert(canonical.clone()) {
        return;
    }
    modules.push(canonical);
}

fn search_modules(
    modules: &[PathBuf],
    query: &str,
    kind_filter: Option<KindFilter>,
) -> Vec<FindResult> {
    let query_tokens = tokenize(query);
    if query_tokens.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<FindResult> = modules
        .iter()
        .flat_map(|module| collect_candidates(module))
        .filter(|candidate| {
            kind_filter.is_none_or(|filter| candidate.kind.as_str() == filter.as_str())
        })
        .filter_map(|candidate| score_candidate(candidate, &query_tokens))
        .collect();

    results.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.path.cmp(&right.path))
    });
    results
}

fn collect_candidates(module_root: &Path) -> Vec<Candidate> {
    let module_name = module_name(module_root);
    let mut candidates = Vec::new();
    candidates.extend(collect_flat_kind(
        module_root,
        &module_name,
        ContentKind::Agents,
    ));
    candidates.extend(collect_flat_kind(
        module_root,
        &module_name,
        ContentKind::Rules,
    ));
    candidates.extend(collect_skills(module_root, &module_name));
    candidates
}

fn collect_flat_kind(module_root: &Path, module_name: &str, kind: ContentKind) -> Vec<Candidate> {
    let mut files = Vec::new();
    collect_markdown_files(&module_root.join(kind.as_str()), &mut files);
    files
        .into_iter()
        .filter_map(|path| candidate_from_file(module_root, module_name, kind, &path))
        .collect()
}

fn collect_markdown_files(dir: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with('.'))
            {
                continue;
            }
            collect_markdown_files(&path, files);
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            files.push(path);
        }
    }
}

fn collect_skills(module_root: &Path, module_name: &str) -> Vec<Candidate> {
    let skills_root = module_root.join("skills");
    let Ok(entries) = fs::read_dir(&skills_root) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|entry| entry.path().join("SKILL.md"))
        .filter(|path| path.is_file())
        .filter_map(|path| {
            candidate_from_file(module_root, module_name, ContentKind::Skills, &path)
        })
        .collect()
}

fn candidate_from_file(
    module_root: &Path,
    module_name: &str,
    kind: ContentKind,
    path: &Path,
) -> Option<Candidate> {
    let content = fs::read_to_string(path).ok()?;
    let fallback_name = fallback_name(kind, path)?;
    let name = parse::frontmatter_value(&content, "name").unwrap_or(fallback_name);
    let mut description = String::new();
    if let Some(value) = parse::frontmatter_value(&content, "description") {
        description = value;
    }
    let triggers = trigger_text(&content);
    Some(Candidate {
        name,
        kind,
        module: module_name.to_string(),
        path: path.strip_prefix(module_root).unwrap_or(path).to_path_buf(),
        description,
        triggers,
    })
}

fn fallback_name(kind: ContentKind, path: &Path) -> Option<String> {
    if kind == ContentKind::Skills {
        return path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_string());
    }
    path.file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
}

fn trigger_text(content: &str) -> String {
    let mut parts = Vec::new();
    for key in ["when_to_use", "use_when", "trigger", "triggers", "USE-WHEN"] {
        if let Some(value) = parse::frontmatter_value(content, key) {
            parts.push(value);
        }
        if let Some(value) = parse::frontmatter_list(content, key) {
            parts.push(value);
        }
    }
    let body = parse::frontmatter_body(content);
    for line in body.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("use when") || lower.contains("use-when") || lower.contains("trigger") {
            parts.push(line.to_string());
        }
    }
    parts.join(" ")
}

fn score_candidate(candidate: Candidate, query_tokens: &HashSet<String>) -> Option<FindResult> {
    let name_tokens = tokenize(&candidate.name);
    let trigger_tokens = tokenize(&candidate.triggers);
    let description_tokens = tokenize(&candidate.description);

    let name_hits = overlap(query_tokens, &name_tokens);
    let trigger_hits = overlap(query_tokens, &trigger_tokens);
    let description_hits = overlap(query_tokens, &description_tokens);
    let score = f64::from(name_hits * 3 + trigger_hits * 2 + description_hits);
    if score == 0.0 {
        return None;
    }

    Some(FindResult {
        name: candidate.name,
        kind: candidate.kind.as_str().to_string(),
        module: candidate.module,
        path: path_to_slash(&candidate.path),
        description: candidate.description,
        score,
        source: "local".to_string(),
    })
}

fn tokenize(value: &str) -> HashSet<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn overlap(query: &HashSet<String>, field: &HashSet<String>) -> u32 {
    query
        .iter()
        .filter(|token| field.contains(*token))
        .count()
        .try_into()
        .unwrap_or(u32::MAX)
}

fn module_name(module_root: &Path) -> String {
    match commands::module::load(module_root) {
        Ok(manifest) => manifest.name,
        Err(_) => module_root.file_name().map_or_else(
            || "module".to_string(),
            |name| name.to_string_lossy().to_string(),
        ),
    }
}

fn print_json(results: &[FindResult]) -> Result<(), String> {
    let json = serde_json::to_string_pretty(results)
        .map_err(|error| format!("cannot serialize find results: {error}"))?;
    println!("{json}");
    Ok(())
}

fn print_console(results: &[FindResult]) {
    if results.is_empty() {
        println!("No matches.");
        return;
    }
    for result in results {
        println!(
            "{:.1}  {}  {}  {}",
            result.score, result.kind, result.name, result.path
        );
        if !result.description.is_empty() {
            println!("      {}", result.description);
        }
    }
}

fn path_to_slash(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(segment) => Some(segment.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
