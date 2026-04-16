use commands::assemble;
use commands::error::Error;
use std::collections::HashMap;
use std::path::Path;

use super::sources::SourceFile;
use crate::cli::config::read_file;

/// Run the assembly pipeline on a single source file.
///
/// For passthrough files (non-SKILL.md companions), returns content unchanged.
/// For assembled files:
///   1. Resolves variant overrides for the target provider
///   2. Runs frontmatter stripping with `keep_fields`
///   3. Strips reference-style links
///   4. Applies tool name remapping
///
/// ```text
/// source: rules/MyRule.md (with frontmatter, references)
///   → variant resolution (provider-specific overrides)
///   → frontmatter stripped (keep_fields applied)
///   → references stripped
///   → tool names remapped (Read → read_file for gemini)
/// ```
pub fn assemble_source(
    source: &SourceFile,
    module_root: &Path,
    provider_name: &str,
    keep_fields: &[String],
    model_tiers: &HashMap<String, Vec<String>>,
    strip_links: bool,
) -> Result<String, Error> {
    if source.passthrough {
        return Ok(source.content.clone());
    }

    let source_dir = Path::new(&source.full_path).parent().unwrap_or(module_root);
    let filename = extract_filename(&source.full_path);

    let variant = assemble::variants::resolve(source_dir, &filename, &[provider_name.to_string()]);

    let variant_content = match &variant {
        Some(vp) => Some(read_file(vp)?),
        None => None,
    };

    let keep_refs: Vec<&str> = keep_fields.iter().map(String::as_str).collect();

    let mut output = assemble::assemble(
        &source.content,
        variant_content.as_deref(),
        &keep_refs,
        strip_links,
    );

    // Map abstract model tiers (strong/fast/light) to provider-specific values
    if source.kind == commands::provider::ContentKind::Agents && !model_tiers.is_empty() {
        output = map_model_tier(&output, model_tiers);
    }

    Ok(output)
}

/// Replace `model: <tier>` in frontmatter with the provider-specific model name.
///
/// Given `model: strong` and tier mapping `{strong: [opus, sonnet]}`, produces `model: opus`.
/// If the model value isn't a known tier, it passes through unchanged.
fn map_model_tier(content: &str, tiers: &HashMap<String, Vec<String>>) -> String {
    assemble::map_field(content, "model", |current_value| {
        tiers
            .get(current_value.trim())
            .and_then(|models| models.first())
            .cloned()
            .unwrap_or_else(|| current_value.to_string())
    })
}

/// Extract the filename component from a path string.
fn extract_filename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
