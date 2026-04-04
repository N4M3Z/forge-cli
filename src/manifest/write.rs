use super::ManifestEntry;
use std::collections::HashMap;

/// Serialize manifest entries as a nested YAML tree mirroring the directory structure.
///
/// Flat keys like `rules/cz/PersonalTaxIncome.md` become nested maps:
///
/// ```yaml
/// rules:
///     cz:
///         PersonalTaxIncome.md:
///             fingerprint: abc123...
///             provenance: .provenance/PersonalTaxIncome.yaml
/// ```
pub fn write(entries: &HashMap<String, ManifestEntry>) -> Result<String, String> {
    let mut root = serde_yaml::Mapping::new();

    let mut sorted_keys: Vec<&String> = entries.keys().collect();
    sorted_keys.sort();

    for key in sorted_keys {
        let entry = &entries[key];
        let segments: Vec<&str> = key.split('/').collect();
        insert_nested(&mut root, &segments, entry);
    }

    serde_yaml::to_string(&root).map_err(|error| format!("failed to serialize manifest: {error}"))
}

/// Insert a manifest entry into the nested YAML tree at the path described by segments.
fn insert_nested(current: &mut serde_yaml::Mapping, segments: &[&str], entry: &ManifestEntry) {
    if segments.len() == 1 {
        let mut fields = serde_yaml::Mapping::new();
        fields.insert(
            serde_yaml::Value::String("fingerprint".into()),
            serde_yaml::Value::String(entry.fingerprint.clone()),
        );
        if let Some(provenance) = &entry.provenance {
            fields.insert(
                serde_yaml::Value::String("provenance".into()),
                serde_yaml::Value::String(provenance.clone()),
            );
        }
        current.insert(
            serde_yaml::Value::String(segments[0].to_string()),
            serde_yaml::Value::Mapping(fields),
        );
        return;
    }

    let directory_key = serde_yaml::Value::String(segments[0].to_string());

    let child = current
        .entry(directory_key)
        .or_insert_with(|| serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));

    if let serde_yaml::Value::Mapping(child_map) = child {
        insert_nested(child_map, &segments[1..], entry);
    }
}
