use commands::error::Error;
use commands::result::ActionResult;

use super::assemble;
use super::copy;

/// Assemble and deploy module content to provider directories.
///
/// Composes `assemble` and `copy` into a single operation:
///
/// ```text
/// 1. assemble(path)              → build/ populated
/// 2. copy(path, force, interactive) → build/ → provider targets
/// 3. merge both ActionResults
/// ```
pub fn execute(
    path: &str,
    target: Option<&str>,
    force: bool,
    interactive: bool,
) -> Result<ActionResult, Error> {
    let mut assemble_result = assemble::execute(path)?;
    let copy_result = copy::execute(path, target, force, interactive)?;

    assemble_result.installed.extend(copy_result.installed);
    assemble_result.skipped.extend(copy_result.skipped);
    assemble_result.errors.extend(copy_result.errors);

    Ok(assemble_result)
}
