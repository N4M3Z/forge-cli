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
pub fn execute(
    path: &str,
    target: Option<&str>,
    force: bool,
    prune: bool,
    interactive: bool,
) -> Result<ActionResult, Error> {
    assemble::execute(path)?;
    deploy::execute(path, target, force, prune, interactive)
}

#[cfg(test)]
mod tests;
