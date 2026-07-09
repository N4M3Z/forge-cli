use commands::error::Error;
use commands::error::ErrorKind;
use commands::result::ActionResult;
use std::path::Path;

use super::assemble;
use super::config;
use super::deploy;

struct StaleSource {
    trunk: String,
    commits_behind: usize,
}

/// Assemble and deploy module content to provider directories.
///
/// ```text
/// 1. assemble(path)    → build/ populated
/// 2. deploy(path)      → build/ → provider targets
/// ```
///
/// Returns only the deployment result — assembly is an internal step.
#[allow(clippy::fn_params_excessive_bools, clippy::too_many_arguments)]
pub fn execute(
    path: &str,
    target: Option<&str>,
    requested_providers: &[String],
    force: bool,
    prune: bool,
    interactive: bool,
    dry_run: bool,
    model: Option<&str>,
    allow_stale: bool,
) -> Result<ActionResult, Error> {
    warn_or_block_stale_source(Path::new(path), allow_stale)?;
    assemble::execute_with_model(path, model)?;
    deploy::execute(
        path,
        target,
        requested_providers,
        force,
        prune,
        interactive,
        dry_run,
    )
}

fn warn_or_block_stale_source(module_root: &Path, allow_stale: bool) -> Result<(), Error> {
    let module_label = module_label(module_root);
    let stale = match detect_stale_source(module_root) {
        Ok(Some(stale)) => stale,
        Ok(None) => return Ok(()),
        Err(error) => {
            eprintln!(
                "warning: cannot determine git freshness for {module_label}: {error}; continuing"
            );
            return Ok(());
        }
    };

    let count = match stale.commits_behind {
        1 => "1 commit".to_string(),
        count => format!("{count} commits"),
    };
    let warning = format!(
        "WARNING: source module {module_label} is {count} behind {}; deploying it may resurrect stale content",
        stale.trunk
    );
    eprintln!("{warning}");

    if allow_stale {
        return Ok(());
    }

    Err(Error::new(
        ErrorKind::Config,
        format!("{warning}. Refusing to install; pass --allow-stale to continue."),
    ))
}

fn module_label(module_root: &Path) -> String {
    let source_uri = config::load_source_uri(module_root);
    if source_uri.is_empty() {
        module_root.display().to_string()
    } else {
        source_uri
    }
}

fn detect_stale_source(module_root: &Path) -> Result<Option<StaleSource>, String> {
    if !module_root.join(".git").exists() {
        return Ok(None);
    }

    let repo = gix::open(module_root).map_err(|error| error.to_string())?;
    let head_id = repo.head_id().map_err(|error| error.to_string())?.detach();

    // Deliberately do not fetch here. `forge install` compares against the
    // last-known origin trunk ref so the check stays cheap and offline-safe.
    let Some((trunk_name, trunk_ref)) = trunk_reference(&repo)? else {
        return Ok(None);
    };
    let trunk_id = trunk_ref
        .into_fully_peeled_id()
        .map_err(|error| error.to_string())?;

    if trunk_id == head_id {
        return Ok(None);
    }

    for (commits_behind, item) in trunk_id
        .ancestors()
        .all()
        .map_err(|error| error.to_string())?
        .enumerate()
    {
        let info = item.map_err(|error| error.to_string())?;
        if info.id == head_id {
            return Ok(Some(StaleSource {
                trunk: trunk_name.to_string(),
                commits_behind,
            }));
        }
    }

    Ok(None)
}

fn trunk_reference(
    repo: &gix::Repository,
) -> Result<Option<(&'static str, gix::Reference<'_>)>, String> {
    for name in ["refs/remotes/origin/main", "refs/remotes/origin/master"] {
        if let Some(reference) = repo
            .try_find_reference(name)
            .map_err(|error| error.to_string())?
        {
            return Ok(Some((name, reference)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests;
