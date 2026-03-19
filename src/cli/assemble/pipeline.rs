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
    tool_mappings: &HashMap<String, String>,
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

    let mut output = assemble::assemble(&source.content, variant_content.as_deref(), &keep_refs);

    if !tool_mappings.is_empty() {
        for (from, to) in tool_mappings {
            output = output.replace(from, to);
        }
    }

    Ok(output)
}

/// Extract the filename component from a path string.
fn extract_filename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}
