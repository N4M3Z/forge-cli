mod kebab_case;
mod remap_tools;
mod to_toml;

pub use kebab_case::to_kebab_case;
pub use remap_tools::remap_tools;
pub use to_toml::markdown_to_toml;

use std::collections::HashMap;

use crate::provider::AssemblyRule;

/// Apply assembly rules to content and filename in order.
///
/// Returns the transformed `(content, filename)` pair.
///
/// Rules are applied sequentially:
///   - `KebabCase` — transforms filename from `PascalCase` to kebab-case
///   - `RemapTools` — replaces tool names in backtick spans
///   - `AgentsToToml` — converts markdown body to TOML, changes `.md` to `.toml`
pub fn apply_rules(
    content: &str,
    filename: &str,
    rules: &[AssemblyRule],
    tool_mappings: &HashMap<String, String>,
) -> Result<(String, String), String> {
    let mut current_content = content.to_string();
    let mut current_filename = filename.to_string();

    for rule in rules {
        match rule {
            AssemblyRule::KebabCase => {
                let (stem, extension) = split_extension(&current_filename);
                let kebab = to_kebab_case(&stem);
                current_filename = format!("{kebab}{extension}");
            }
            AssemblyRule::RemapTools => {
                current_content = remap_tools(&current_content, tool_mappings);
            }
            AssemblyRule::AgentsToToml => {
                current_content = markdown_to_toml(&current_filename, &current_content)?;
                let (stem, _) = split_extension(&current_filename);
                current_filename = format!("{stem}.toml");
            }
            AssemblyRule::StripLinks => {}
        }
    }

    Ok((current_content, current_filename))
}

/// Split a filename into stem and extension (including the dot).
///
/// `SecurityArchitect.md` → (`SecurityArchitect`, `.md`)
/// `no-extension`         → (`no-extension`, empty string)
fn split_extension(filename: &str) -> (String, String) {
    if let Some(dot_pos) = filename.rfind('.') {
        let stem = filename[..dot_pos].to_string();
        let extension = filename[dot_pos..].to_string();
        (stem, extension)
    } else {
        (filename.to_string(), String::new())
    }
}

#[cfg(test)]
mod tests;
