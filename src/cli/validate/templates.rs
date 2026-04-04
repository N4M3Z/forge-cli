use rust_embed::RustEmbed;
use std::fs;
use std::path::Path;

#[derive(RustEmbed)]
#[folder = "templates/"]
struct SchemaTemplates;

/// Map content kind directory names to their template schema filenames.
fn template_name(kind: &str) -> Option<&'static str> {
    match kind {
        "skills" => Some("skills.mdschema"),
        "agents" => Some("agents.mdschema"),
        "rules" => Some("rules.mdschema"),
        "decisions" => Some("decisions.mdschema"),
        _ => None,
    }
}

/// Scaffold a missing `.mdschema` from the embedded template.
///
/// Returns the schema content if scaffolded, None if no template exists
/// for this content kind or the schema already exists.
pub fn scaffold_if_missing(dir: &Path, kind: &str) -> Option<String> {
    let mdschema_path = dir.join(".mdschema");
    if mdschema_path.exists() {
        return None;
    }

    let template_filename = template_name(kind)?;
    let template_content = SchemaTemplates::get(template_filename)?;
    let content = std::str::from_utf8(template_content.data.as_ref()).ok()?;

    if let Err(error) = fs::write(&mdschema_path, content) {
        eprintln!(
            "  WARN  could not scaffold {}: {error}",
            mdschema_path.display()
        );
        return None;
    }

    eprintln!(
        "  scaffold  {} (from template {template_filename})",
        mdschema_path.display()
    );
    Some(content.to_string())
}
