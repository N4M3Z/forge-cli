use super::ManifestEntry;
use std::collections::HashMap;

pub fn write(entries: &HashMap<String, ManifestEntry>) -> Result<String, String> {
    let mut top: serde_yaml::Mapping = serde_yaml::Mapping::new();

    for (name, entry) in entries {
        let mut fields = serde_yaml::Mapping::new();
        fields.insert(
            serde_yaml::Value::String("sha256".into()),
            serde_yaml::Value::String(entry.sha256.clone()),
        );
        top.insert(
            serde_yaml::Value::String(name.clone()),
            serde_yaml::Value::Mapping(fields),
        );
    }

    serde_yaml::to_string(&top).map_err(|e| format!("failed to serialize manifest: {e}"))
}
