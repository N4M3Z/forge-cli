//! Per-repo version-control state: branch, ahead/behind, dirty paths, and
//! jj colocation. One `git status` per repo; artifacts are matched against
//! the dirty set by path.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::view::{VcsState, WorktreeState};

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
    let (behind, ahead) = git_stdout(
        module_dir,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    )
    .and_then(|out| parse_counts(&out))
    .unwrap_or((0, 0));
    let status = git_stdout(module_dir, &["status", "--porcelain"]).unwrap_or_default();
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

/// `git rev-list --left-right --count @{upstream}...HEAD` prints
/// `<behind>\t<ahead>`: left side counts commits only on the upstream.
fn parse_counts(raw: &str) -> Option<(usize, usize)> {
    let mut fields = raw.split_whitespace();
    let behind = fields.next()?.parse().ok()?;
    let ahead = fields.next()?.parse().ok()?;
    Some((behind, ahead))
}

/// Splits porcelain v1 status lines into (dirty, untracked) path sets.
/// Renames keep the new path; quoted paths are unquoted.
fn parse_status(raw: &str) -> (HashSet<String>, HashSet<String>) {
    let mut dirty = HashSet::new();
    let mut untracked = HashSet::new();
    for line in raw.lines() {
        if line.len() < 4 {
            continue;
        }
        let (code, rest) = line.split_at(3);
        let path = rest
            .rsplit(" -> ")
            .next()
            .unwrap_or(rest)
            .trim_matches('"')
            .to_string();
        if code.starts_with("??") {
            untracked.insert(path);
        } else {
            dirty.insert(path);
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
            parse_status(" M agents/Analyst.md\n?? skills/NewSkill/\nR  old.md -> rules/new.md\n");
        assert!(dirty.contains("agents/Analyst.md"));
        assert!(dirty.contains("rules/new.md"));
        assert!(untracked.contains("skills/NewSkill/"));
        assert!(!dirty.contains("old.md"));
    }

    #[test]
    fn untracked_directory_covers_children() {
        let (dirty, untracked) = parse_status("?? skills/NewSkill/\n");
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
