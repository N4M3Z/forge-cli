//! Fetch git sources declared in `.forge` into a content-addressed cache and
//! materialize the requested commit's tree into a worktree-shaped directory
//! that downstream pipeline code treats as a regular module on disk.
//!
//! Cache layout:
//!
//! ```text
//! ~/.cache/forge/git/
//!     <host>/<owner>/<repo>/
//!         .bare.git/      bare clone, fetched once and reused
//!         <commit-sha>/   materialized worktree for one pinned SHA
//! ```
//!
//! The bare clone holds all objects ever fetched from the remote. Per-SHA
//! worktrees are cheap copies of just the tree at that commit, materialized
//! on demand.

use commands::error::{Error, ErrorKind};
use std::path::{Path, PathBuf};

pub fn ensure_cached(url: &str, commit: &str, source_label: &str) -> Result<PathBuf, Error> {
    let cache_root = cache_root()?;
    let (host, owner, repo) = parse_url(url, source_label)?;
    let module_root = cache_root.join(&host).join(&owner).join(&repo);
    let bare_dir = module_root.join(".bare.git");
    let work_dir = module_root.join(commit);

    if work_dir.join("module.yaml").is_file() {
        return Ok(work_dir);
    }

    std::fs::create_dir_all(&module_root).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!(
                "cannot create git cache dir {}: {error}",
                module_root.display()
            ),
        )
    })?;

    if !bare_dir.exists() {
        bare_clone(url, &bare_dir, source_label)?;
    }

    materialize_commit(&bare_dir, commit, &work_dir, source_label)?;
    Ok(work_dir)
}

/// Returns the already-materialized worktree for a pinned commit if it exists
/// on disk, without any network access. For callers that must never trigger a
/// clone inside a request handler (the dashboard scan); an uncached entry
/// resolves to `None` and is skipped.
#[cfg_attr(not(feature = "dashboard"), allow(dead_code))]
pub fn cached_worktree(url: &str, commit: &str, source_label: &str) -> Option<PathBuf> {
    let cache_root = cache_root().ok()?;
    let (host, owner, repo) = parse_url(url, source_label).ok()?;
    let work_dir = cache_root.join(host).join(owner).join(repo).join(commit);
    if work_dir.join("module.yaml").is_file() {
        Some(work_dir)
    } else {
        None
    }
}

fn cache_root() -> Result<PathBuf, Error> {
    if let Ok(override_dir) = std::env::var("FORGE_GIT_CACHE_DIR") {
        return Ok(PathBuf::from(override_dir));
    }
    let base = dirs::cache_dir().ok_or_else(|| {
        Error::new(
            ErrorKind::Io,
            "cannot resolve user cache directory (set XDG_CACHE_HOME or HOME)".to_string(),
        )
    })?;
    Ok(base.join("forge").join("git"))
}

fn parse_url(url: &str, source_label: &str) -> Result<(String, String, String), Error> {
    let after_scheme = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("file://"))
        .ok_or_else(|| {
            Error::new(
                ErrorKind::Config,
                format!(".forge: source '{source_label}' has unsupported URL scheme: {url}"),
            )
        })?;
    let mut parts = after_scheme.trim_start_matches('/').split('/');
    let host = parts.next().unwrap_or_default().to_string();
    let owner = parts.next().unwrap_or_default().to_string();
    let repo_raw = parts.next().unwrap_or_default();
    let repo = repo_raw.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        return Err(Error::new(
            ErrorKind::Config,
            format!(
                ".forge: source '{source_label}' URL must be <scheme>://<host>/<owner>/<repo>, got: {url}"
            ),
        ));
    }
    let host_safe = if host.is_empty() {
        "_local".to_string()
    } else {
        host
    };
    Ok((host_safe, owner, repo))
}

fn bare_clone(url: &str, bare_dir: &Path, source_label: &str) -> Result<(), Error> {
    let parsed = gix::Url::try_from(url).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(".forge: source '{source_label}' invalid git URL '{url}': {error}"),
        )
    })?;
    let mut prepare = gix::clone::PrepareFetch::new(
        parsed,
        bare_dir,
        gix::create::Kind::Bare,
        gix::create::Options::default(),
        gix::open::Options::isolated(),
    )
    .map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!(".forge: git clone setup for '{source_label}' failed: {error}"),
        )
    })?;
    prepare
        .fetch_only(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!(".forge: git fetch for '{source_label}' from {url} failed: {error}"),
            )
        })?;
    Ok(())
}

fn materialize_commit(
    bare_dir: &Path,
    commit: &str,
    work_dir: &Path,
    source_label: &str,
) -> Result<(), Error> {
    let repo = gix::open(bare_dir).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot open git cache at {}: {error}", bare_dir.display()),
        )
    })?;

    let oid = gix::ObjectId::from_hex(commit.as_bytes()).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(".forge: source '{source_label}' invalid commit SHA '{commit}': {error}"),
        )
    })?;

    let commit_obj = repo.find_object(oid).map_err(|error| {
        Error::new(
            ErrorKind::Config,
            format!(
                ".forge: source '{source_label}' commit {commit} not found in repository (was it pushed and is it reachable from the default branch?): {error}"
            ),
        )
    })?;

    let tree = commit_obj
        .peel_to_kind(gix::object::Kind::Tree)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot resolve tree for commit {commit}: {error}"),
            )
        })?;

    std::fs::create_dir_all(work_dir).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot create worktree dir {}: {error}", work_dir.display()),
        )
    })?;

    let mut index_file = repo.index_from_tree(&tree.id).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot build index from tree {}: {error}", tree.id),
        )
    })?;

    let opts = gix::worktree::state::checkout::Options::default();
    gix::worktree::state::checkout(
        &mut index_file,
        work_dir,
        repo.objects.clone().into_arc().map_err(|error| {
            Error::new(ErrorKind::Io, format!("cannot share object store: {error}"))
        })?,
        &gix::progress::Discard,
        &gix::progress::Discard,
        &gix::interrupt::IS_INTERRUPTED,
        opts,
    )
    .map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!(
                "git checkout of {commit} into {} failed: {error}",
                work_dir.display()
            ),
        )
    })?;

    Ok(())
}
