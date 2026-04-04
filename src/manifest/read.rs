use super::ManifestEntry;
use std::collections::HashMap;

pub fn read(manifest_content: &str) -> Result<HashMap<String, ManifestEntry>, String> {
    let raw: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(manifest_content)
        .map_err(|e| format!("failed to parse manifest YAML: {e}"))?;

    let mut entries = HashMap::new();

    for (name, value) in raw {
        let sha256 = super::extract::string_field(&value, "sha256", &name)?;
        entries.insert(name, ManifestEntry { sha256 });
    }

    Ok(entries)
}
