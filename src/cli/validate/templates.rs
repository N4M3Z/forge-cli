use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "templates/"]
struct SchemaTemplates;

fn template_name(kind: &str) -> Option<&'static str> {
    match kind {
        "skills" => Some("mdschema/skills.mdschema"),
        "agents" => Some("mdschema/agents.mdschema"),
        "rules" => Some("mdschema/rules.mdschema"),
        "decisions" => Some("mdschema/decisions.mdschema"),
        _ => None,
    }
}

pub fn embedded_mdschema(kind: &str) -> Option<String> {
    let template_filename = template_name(kind)?;
    let template_content = SchemaTemplates::get(template_filename)?;
    let content = std::str::from_utf8(template_content.data.as_ref()).ok()?;
    Some(content.to_string())
}
