use super::ManifestEntry;
use std::collections::HashMap;

pub fn write(entries: &HashMap<String, ManifestEntry>) -> Result<String, String> {
    let mut top: serde_yaml::Mapping = serde_yaml::Mapping::new();

    let mut sorted_keys: Vec<&String> = entries.keys().collect();
    sorted_keys.sort();

    for name in sorted_keys {
        let entry = &entries[name];
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
