//! Per-repo version-control state: branch, ahead/behind, dirty paths, and
//! jj colocation. One `git status` per repo; artifacts are matched against
//! the dirty set by path.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::history::{GIT_LOG_FORMAT, enrich_commits_with_entire, parse_git_log};
use crate::view::{GitCommit, VcsState, WorktreeState};

pub(super) struct RepoVcs {
    branch: String,
    ahead: usize,
    behind: usize,
    jj_colocated: bool,
    /// Module directory relative to the repo root — empty today (module dir
    /// is the root), non-empty once modules live inside a monorepo.
    prefix: PathBuf,
    dirty: HashSet<String>,
    untracked: HashSet<String>,
}

impl RepoVcs {
    pub(super) fn state_for(&self, relative_path: &str) -> VcsState {
        let repo_relative = self
            .prefix
            .join(relative_path)
            .to_string_lossy()
            .into_owned();
        let worktree = if self.covers_untracked(&repo_relative) {
            WorktreeState::Untracked
        } else if self.dirty.contains(&repo_relative) {
            WorktreeState::Modified
        } else {
            WorktreeState::Clean
        };
        VcsState {
            branch: self.branch.clone(),
            worktree,
            ahead: self.ahead,
            behind: self.behind,
            jj_colocated: self.jj_colocated,
        }
    }

    /// Repo-level state for the module row: modified when anything under the
    /// module's prefix is dirty or untracked.
    pub(super) fn module_state(&self) -> VcsState {
        let touched = self.dirty.iter().chain(self.untracked.iter()).any(|path| {
            self.prefix.as_os_str().is_empty() || Path::new(path).starts_with(&self.prefix)
        });
        VcsState {
            branch: self.branch.clone(),
            worktree: if touched {
                WorktreeState::Modified
            } else {
                WorktreeState::Clean
            },
            ahead: self.ahead,
            behind: self.behind,
            jj_colocated: self.jj_colocated,
        }
    }

    /// Untracked directories appear in porcelain output as a single entry with
    /// a trailing slash covering everything beneath them.
    fn covers_untracked(&self, repo_relative: &str) -> bool {
        self.untracked.contains(repo_relative)
            || self
                .untracked
                .iter()
                .any(|entry| entry.ends_with('/') && repo_relative.starts_with(entry.as_str()))
    }
}

pub(super) fn repo_vcs(module_dir: &Path) -> Option<RepoVcs> {
    let root_raw = git_stdout(module_dir, &["rev-parse", "--show-toplevel"])?;
    let root = std::fs::canonicalize(root_raw.trim()).ok()?;
    let jj_colocated = root.join(".jj").is_dir();
    let branch = branch_label(module_dir, jj_colocated);
    let (behind, ahead) = upstream_counts(module_dir, &branch);
    let status = git_stdout(module_dir, &["status", "--porcelain", "-z"]).unwrap_or_default();
    let (dirty, untracked) = parse_status(&status);
    let module_canonical = std::fs::canonicalize(module_dir).ok()?;
    let prefix = module_canonical
        .strip_prefix(&root)
        .unwrap_or(Path::new(""))
        .to_path_buf();
    Some(RepoVcs {
        branch,
        ahead,
        behind,
        jj_colocated,
        prefix,
        dirty,
        untracked,
    })
}

/// Jujutsu-colocated repos keep git HEAD detached, so `--abbrev-ref HEAD`
/// answers `HEAD` there. Prefer the jj bookmark on the working-copy parent,
/// then a branch pointing at HEAD, then the short sha.
fn branch_label(dir: &Path, jj_colocated: bool) -> String {
    let named = git_stdout(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .map(|out| out.trim().to_string())
        .unwrap_or_default();
    if !named.is_empty() && named != "HEAD" {
        return named;
    }
    if jj_colocated {
        let bookmark = command_stdout(
            dir,
            "jj",
            &[
                "--ignore-working-copy",
                "log",
                "--no-graph",
                "-r",
                "heads(::@- & bookmarks())",
                "-T",
                "local_bookmarks.join(\",\") ++ \"\\n\"",
            ],
        )
        .and_then(|out| out.lines().next().map(str::to_string))
        .unwrap_or_default();
        if !bookmark.is_empty() {
            return bookmark;
        }
    }
    let pointing = git_stdout(
        dir,
        &[
            "for-each-ref",
            "--points-at",
            "HEAD",
            "--format=%(refname:short)",
            "refs/heads",
        ],
    )
    .and_then(|out| out.lines().next().map(str::to_string))
    .unwrap_or_default();
    if !pointing.is_empty() {
        return pointing;
    }
    git_stdout(dir, &["rev-parse", "--short", "HEAD"])
        .map(|out| format!("detached {}", out.trim()))
        .unwrap_or_default()
}

/// Most recent commits across the whole repo, for the repository detail view.
pub(super) fn repo_log(repo: &Path) -> Vec<GitCommit> {
    let Some(raw) = git_stdout(repo, &["log", "-n", "8", GIT_LOG_FORMAT]) else {
        return Vec::new();
    };
    let mut commits = parse_git_log(&raw);
    enrich_commits_with_entire(repo, &mut commits);
    commits
}

fn git_stdout(dir: &Path, args: &[&str]) -> Option<String> {
    command_stdout(dir, "git", args)
}

fn command_stdout(dir: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Ahead/behind relative to the upstream. `@{upstream}` needs an attached
/// HEAD, which jj-colocated repos never have — fall back to the resolved
/// branch's configured upstream, then to `origin/<branch>`.
fn upstream_counts(dir: &Path, branch: &str) -> (usize, usize) {
    for range in upstream_ranges(branch) {
        let counts = git_stdout(dir, &["rev-list", "--left-right", "--count", &range])
            .and_then(|out| parse_counts(&out));
        if let Some(counts) = counts {
            return counts;
        }
    }
    (0, 0)
}

/// The branch label may carry jj artifacts: several bookmarks joined by
/// commas, or a `??` conflicted-bookmark suffix. Use the first bookmark,
/// stripped, for the fallback ranges.
fn upstream_ranges(branch: &str) -> Vec<String> {
    let mut ranges = vec!["@{upstream}...HEAD".to_string()];
    let cleaned = branch
        .split(',')
        .next()
        .unwrap_or_default()
        .trim_end_matches('?');
    if !cleaned.is_empty() && !cleaned.starts_with("detached ") {
        ranges.push(format!("{cleaned}@{{upstream}}...HEAD"));
        ranges.push(format!("origin/{cleaned}...HEAD"));
    }
    ranges
}

/// `git rev-list --left-right --count @{upstream}...HEAD` prints
/// `<behind>\t<ahead>`: left side counts commits only on the upstream.
fn parse_counts(raw: &str) -> Option<(usize, usize)> {
    let mut fields = raw.split_whitespace();
    let behind = fields.next()?.parse().ok()?;
    let ahead = fields.next()?.parse().ok()?;
    Some((behind, ahead))
}

/// Splits `git status --porcelain -z` output into (dirty, untracked) path
/// sets. NUL separation means paths arrive verbatim — no C-style quoting to
/// decode and no ` -> ` rename ambiguity. A rename or copy entry carries the
/// new path inline and its original path as the following NUL field, which is
/// consumed and dropped.
fn parse_status(raw: &str) -> (HashSet<String>, HashSet<String>) {
    let mut dirty = HashSet::new();
    let mut untracked = HashSet::new();
    let mut fields = raw.split('\0');
    while let Some(entry) = fields.next() {
        if entry.len() < 4 || !entry.is_char_boundary(3) {
            continue;
        }
        let (code, path) = entry.split_at(3);
        if code.starts_with("??") {
            untracked.insert(path.to_string());
            continue;
        }
        dirty.insert(path.to_string());
        if code.contains('R') || code.contains('C') {
            let _original_path = fields.next();
        }
    }
    (dirty, untracked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_status_separates_dirty_and_untracked() {
        let (dirty, untracked) =
            parse_status(" M agents/Analyst.md\0?? skills/NewSkill/\0R  rules/new.md\0old.md\0");
        assert!(dirty.contains("agents/Analyst.md"));
        assert!(dirty.contains("rules/new.md"));
        assert!(untracked.contains("skills/NewSkill/"));
        assert!(!dirty.contains("old.md"));
    }

    #[test]
    fn parse_status_keeps_special_characters_verbatim() {
        let (dirty, _) = parse_status(" M skills/Nový/SKILL.md\0 M skills/a -> b/SKILL.md\0");
        assert!(dirty.contains("skills/Nový/SKILL.md"));
        assert!(dirty.contains("skills/a -> b/SKILL.md"));
    }

    #[test]
    fn untracked_directory_covers_children() {
        let (dirty, untracked) = parse_status("?? skills/NewSkill/\0");
        let repo = RepoVcs {
            branch: "main".to_string(),
            ahead: 0,
            behind: 0,
            jj_colocated: false,
            prefix: PathBuf::new(),
            dirty,
            untracked,
        };
        let state = repo.state_for("skills/NewSkill/SKILL.md");
        assert_eq!(state.worktree, WorktreeState::Untracked);
        let clean = repo.state_for("skills/OldSkill/SKILL.md");
        assert_eq!(clean.worktree, WorktreeState::Clean);
    }

    #[test]
    fn parse_counts_reads_behind_then_ahead() {
        assert_eq!(parse_counts("1\t3\n"), Some((1, 3)));
        assert_eq!(parse_counts("garbage"), None);
    }
}
