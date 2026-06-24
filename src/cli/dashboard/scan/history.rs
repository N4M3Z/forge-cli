//! Git history, Entire checkpoint enrichment, and deploy-time source recovery.

use super::sidecar::{Sidecar, parse_adoption};
use super::source::resolve_sidecar;
use commands::manifest;
use commands::view::{Adoption, GitCommit};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Reads the first `resolvedDependencies` digest from an `assemble/v1`
/// sidecar (the source input SHA captured at deploy time).
pub(super) fn recorded_input_sha(sidecar_content: &str) -> Option<String> {
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
pub(super) const GIT_LOG_FORMAT: &str =
    "--format=%H%x00%s%x00%ai%x00%an%x00%(trailers:key=Entire-Checkpoint,valueonly,separator=%x20)";

pub(super) fn parse_git_log(raw: &str) -> Vec<GitCommit> {
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
pub(super) fn enrich_commits_with_entire(repo: &Path, commits: &mut [GitCommit]) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
