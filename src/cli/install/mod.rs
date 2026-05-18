use commands::error::Error;
use commands::result::ActionResult;

use super::assemble;
use super::deploy;

/// Assemble and deploy module content to provider directories.
///
/// ```text
/// 1. assemble(path)    → build/ populated
/// 2. deploy(path)      → build/ → provider targets
/// ```
///
/// Returns only the deployment result — assembly is an internal step.
#[allow(clippy::fn_params_excessive_bools)]
pub fn execute(
    path: &str,
    target: Option<&str>,
    requested_providers: &[String],
    force: bool,
    prune: bool,
    interactive: bool,
    dry_run: bool,
) -> Result<ActionResult, Error> {
    assemble::execute(path)?;
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

#[cfg(test)]
mod tests;
