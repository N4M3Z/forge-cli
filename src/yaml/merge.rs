use serde_yaml::Value;

/// Deep-merge two YAML documents. Values in override take precedence.
///
/// Recursive merge for mappings. Sequences and scalars in override replace defaults.
///
/// Given these documents:
///
/// ```yaml
/// # defaults
/// user:
///   root: /default
///   theme: light
/// debug: false
/// ```
///
/// ```yaml
/// # overrides
/// user:
///   root: /custom
/// extra: true
/// ```
///
/// `deep_merge(defaults, overrides)` produces:
///
/// ```yaml
/// user:
///   root: /custom
///   theme: light
/// debug: false
/// extra: true
/// ```
pub fn deep_merge(defaults_content: &str, override_content: &str) -> Result<String, String> {
    let mut base: Value = serde_yaml::from_str(defaults_content)
        .map_err(|e| format!("failed to parse defaults YAML: {e}"))?;

    let overlay: Value = serde_yaml::from_str(override_content)
        .map_err(|e| format!("failed to parse override YAML: {e}"))?;

    merge_value(&mut base, overlay);

    serde_yaml::to_string(&base).map_err(|e| format!("failed to serialize merged YAML: {e}"))
}

/// Recursively merge `overlay` into `base`. Mappings recurse, everything else replaces.
///
/// Type conflicts (e.g. base is a mapping, overlay is a sequence) keep the base
/// value and skip the overlay. This prevents downstream deserialization failures
/// when a module's config uses a different YAML type than the embedded defaults.
fn merge_value(base: &mut Value, overlay: Value) {
    match (&mut *base, overlay) {
        (Value::Mapping(base_map), Value::Mapping(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                match base_map.get_mut(&key) {
                    Some(base_val) => merge_value(base_val, overlay_val),
                    None => {
                        base_map.insert(key, overlay_val);
                    }
                }
            }
        }
        (Value::Mapping(_), _) | (_, Value::Mapping(_)) => {
            eprintln!("warning: config type conflict (mapping vs non-mapping), keeping default");
        }
        (base, overlay) => {
            *base = overlay;
        }
    }
}
