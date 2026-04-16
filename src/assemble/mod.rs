mod frontmatter;
#[allow(clippy::implicit_hasher)]
pub mod pipeline;
pub mod references;
pub mod variants;

pub use frontmatter::{map_field, strip_frontmatter};
pub use references::{extract, strip};
pub use variants::{Mode, apply, resolve};

use crate::parse;

/// Full assembly pipeline: resolve variant, merge, strip frontmatter, strip refs.
///
/// Steps:
///   1. If variant content is present, read its `mode` and merge with source
///   2. Strip frontmatter (keeping only `keep_fields`)
///   3. Strip reference-style links
pub fn assemble(
    source_content: &str,
    variant_content: Option<&str>,
    keep_fields: &[&str],
    strip_links: bool,
) -> String {
    let merged = match variant_content {
        Some(vc) => {
            let mode_str = parse::frontmatter_value(vc, "mode").unwrap_or_default();
            let mode = Mode::parse(&mode_str);
            apply(source_content, vc, mode)
        }
        None => source_content.to_string(),
    };

    let stripped = strip_frontmatter(&merged, keep_fields);
    if strip_links {
        strip(&stripped)
    } else {
        stripped
    }
}

#[cfg(test)]
mod pipeline_tests;
#[cfg(test)]
mod tests;
