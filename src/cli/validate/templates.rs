use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "templates/init/"]
pub(crate) struct InitTemplates;

const README_MDSCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/README.mdschema"
));
const CONTRIBUTING_MDSCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/CONTRIBUTING.mdschema"
));

pub fn embedded_mdschema(kind: &str) -> Option<String> {
    match kind {
        "readme" => Some(README_MDSCHEMA.to_string()),
        "contributing" => Some(CONTRIBUTING_MDSCHEMA.to_string()),
        _ => {
            let embed_path = match kind {
                "skills" => "skills/.mdschema",
                "agents" => "agents/.mdschema",
                "rules" => "rules/.mdschema",
                "decisions" => "docs/decisions/.mdschema",
                _ => return None,
            };
            let content = InitTemplates::get(embed_path)?;
            let text = std::str::from_utf8(content.data.as_ref()).ok()?;
            Some(text.to_string())
        }
    }
}
