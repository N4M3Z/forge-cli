//! `forge watch` manages the registry of module/deployment locations to
//! monitor, beyond `~` and the cwd. Stored at
//! `~/.config/forge/watchlist.yaml`. Each entry is either a local path string
//! or a SHA-pinned remote `{ git: <https-url>, ref: <40-hex-sha> }`, the same
//! shape `.forge` uses; remote entries resolve to an already-cached worktree in
//! the shared git cache and are skipped when absent (resolution never fetches).

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(untagged)]
enum WatchEntry {
    Git {
        git: String,
        #[serde(rename = "ref")]
        reference: String,
    },
    Path(String),
}

impl WatchEntry {
    fn sort_key(&self) -> String {
        match self {
            WatchEntry::Path(path) => path.clone(),
            WatchEntry::Git { git, reference } => format!("{git}#{reference}"),
        }
    }

    fn label(&self) -> String {
        match self {
            WatchEntry::Path(path) => path.clone(),
            WatchEntry::Git { git, reference } => format!("{git}@{reference}"),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct WatchlistConfig {
    #[serde(default)]
    locations: Vec<WatchEntry>,
}

fn config_path() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".config/forge/watchlist.yaml"))
        .ok_or_else(|| "cannot resolve home directory".to_string())
}

/// Reads the watchlist tolerantly: a missing, unreadable, or malformed file
/// yields an empty config (with a warning). For read-only callers (`list` and
/// other monitors) where one bad file must never abort the read.
fn load_lenient_from(path: &Path) -> WatchlistConfig {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return WatchlistConfig::default();
        }
        Err(error) => {
            eprintln!(
                "warning: cannot read {} ({error}), treating watchlist as empty",
                path.display()
            );
            return WatchlistConfig::default();
        }
    };
    match serde_yaml::from_str(&content) {
        Ok(config) => config,
        Err(error) => {
            eprintln!(
                "warning: {} is malformed ({error}), treating watchlist as empty",
                path.display()
            );
            WatchlistConfig::default()
        }
    }
}

/// Reads the watchlist strictly: a missing file is an empty config (so the
/// first `add` still succeeds), but an unreadable or malformed file is an
/// error. For mutating callers, so a corrupt file is never silently
/// overwritten and its existing entries lost.
fn load_strict_from(path: &Path) -> Result<WatchlistConfig, String> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(WatchlistConfig::default());
        }
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    serde_yaml::from_str(&content).map_err(|error| {
        format!(
            "{} is malformed ({error}); refusing to overwrite. Fix or remove the file, then retry.",
            path.display()
        )
    })
}

fn load_lenient() -> WatchlistConfig {
    match config_path() {
        Ok(path) => load_lenient_from(&path),
        Err(error) => {
            eprintln!("warning: {error}, treating watchlist as empty");
            WatchlistConfig::default()
        }
    }
}

fn load_strict() -> Result<WatchlistConfig, String> {
    load_strict_from(&config_path()?)
}

fn save(config: &WatchlistConfig) -> Result<(), String> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    let yaml = serde_yaml::to_string(config).map_err(|error| format!("serialize: {error}"))?;
    fs::write(&path, yaml).map_err(|error| format!("cannot write {}: {error}", path.display()))
}

fn announce(json: bool, message: &str) {
    if json {
        println!("{}", serde_json::json!({ "message": message }));
    } else {
        println!("{message}");
    }
}

fn sort_and_save(config: &mut WatchlistConfig) -> Result<(), String> {
    config.locations.sort_by_key(WatchEntry::sort_key);
    save(config)
}

/// Lists watched locations.
#[allow(clippy::unnecessary_wraps)]
pub fn list(json: bool) -> Result<i32, String> {
    print_locations(&load_lenient().locations, json);
    Ok(0)
}

/// Adds a local path to the watchlist.
pub fn add_path(path: &str, json: bool) -> Result<i32, String> {
    let mut config = load_strict()?;
    if config
        .locations
        .iter()
        .any(|entry| matches!(entry, WatchEntry::Path(existing) if existing == path))
    {
        announce(json, &format!("already watched: {path}"));
        return Ok(0);
    }
    config.locations.push(WatchEntry::Path(path.to_string()));
    sort_and_save(&mut config)?;
    announce(json, &format!("watching: {path}"));
    Ok(0)
}

/// Adds a SHA-pinned remote repo. HTTPS-only, full 40-char lowercase-hex commit required.
pub fn add_git(url: &str, reference: &str, json: bool) -> Result<i32, String> {
    super::dotforge::validate_git_url(url)?;
    super::dotforge::validate_commit_sha(reference)?;
    let mut config = load_strict()?;
    if config.locations.iter().any(|entry| {
        matches!(entry, WatchEntry::Git { git, reference: pinned } if git == url && pinned == reference)
    }) {
        announce(json, &format!("already watched: {url}@{reference}"));
        return Ok(0);
    }
    config.locations.push(WatchEntry::Git {
        git: url.to_string(),
        reference: reference.to_string(),
    });
    sort_and_save(&mut config)?;
    announce(json, &format!("watching: {url}@{reference}"));
    Ok(0)
}

/// Removes a watched entry by its path or git URL.
pub fn remove(target: &str, json: bool) -> Result<i32, String> {
    let mut config = load_strict()?;
    let before = config.locations.len();
    config.locations.retain(|entry| match entry {
        WatchEntry::Path(path) => path != target,
        WatchEntry::Git { git, .. } => git != target,
    });
    if config.locations.len() == before {
        return Err(format!("not watched: {target}"));
    }
    save(&config)?;
    announce(json, &format!("removed: {target}"));
    Ok(0)
}

fn print_locations(locations: &[WatchEntry], json: bool) {
    if json {
        let labels: Vec<String> = locations.iter().map(WatchEntry::label).collect();
        println!("{}", serde_json::json!({ "locations": labels }));
        return;
    }
    if locations.is_empty() {
        println!("No watched locations. Add one with: forge watch add <path>");
        return;
    }
    for entry in locations {
        match entry {
            WatchEntry::Path(path) => {
                let marker = if resolve(path).is_some_and(|resolved| resolved.exists()) {
                    "ok"
                } else {
                    "missing"
                };
                println!("{path}  [{marker}]");
            }
            WatchEntry::Git { git, reference } => {
                println!("{git} @ {reference}  [git]");
            }
        }
    }
}

/// Expands a leading `~/` and returns the resolved path.
fn resolve(location: &str) -> Option<PathBuf> {
    if let Some(rest) = location.strip_prefix("~/") {
        return dirs::home_dir().map(|home| home.join(rest));
    }
    Some(PathBuf::from(location))
}

/// Reads the watched locations (expanded), for tools that monitor them. Remote
/// git entries resolve only to an already-cached worktree (no network); an
/// entry that has not been fetched yet is skipped. Cloning is never triggered
/// here, so callers inside a request handler stay side-effect-free.
#[must_use]
#[allow(dead_code)]
pub fn watched_locations() -> Vec<PathBuf> {
    load_lenient()
        .locations
        .iter()
        .filter_map(resolve_entry)
        .collect()
}

#[allow(dead_code)]
fn resolve_entry(entry: &WatchEntry) -> Option<PathBuf> {
    match entry {
        WatchEntry::Path(path) => resolve(path),
        WatchEntry::Git { git, reference } => super::dotforge::cached_worktree(git, reference, git),
    }
}

#[cfg(test)]
mod tests;
