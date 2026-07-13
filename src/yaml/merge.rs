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

    merge_value(&mut base, overlay, "");

    serde_yaml::to_string(&base).map_err(|e| format!("failed to serialize merged YAML: {e}"))
}

/// Recursively merge `overlay` into `base`. Mappings recurse, everything else replaces.
///
/// Type conflicts (e.g. base is a mapping, overlay is a sequence) keep the base
/// value and skip the overlay. This prevents downstream deserialization failures
/// when a module's config uses a different YAML type than the embedded defaults.
fn merge_value(base: &mut Value, overlay: Value, key_path: &str) {
    match (&mut *base, overlay) {
        (Value::Mapping(base_map), Value::Mapping(overlay_map)) => {
            for (key, overlay_val) in overlay_map {
                let key_label = key.as_str().map_or_else(|| "?".to_string(), str::to_string);
                let nested_path = if key_path.is_empty() {
                    key_label
                } else {
                    format!("{key_path}.{key_label}")
                };
                match base_map.get_mut(&key) {
                    Some(base_val) => merge_value(base_val, overlay_val, &nested_path),
                    None => {
                        base_map.insert(key, overlay_val);
                    }
                }
            }
        }
        (base_value @ Value::Mapping(_), overlay_value) => {
            if replace_on_type_conflict(key_path) {
                *base_value = overlay_value;
            } else {
                warn_type_conflict(key_path, "mapping", describe_value(&overlay_value));
            }
        }
        (base_value, overlay_value @ Value::Mapping(_)) => {
            if replace_on_type_conflict(key_path) {
                *base_value = overlay_value;
            } else {
                warn_type_conflict(key_path, describe_value(base_value), "mapping");
            }
        }
        (base_value, overlay) => {
            *base_value = overlay;
        }
    }
}

fn replace_on_type_conflict(key_path: &str) -> bool {
    key_path.starts_with("providers.") && key_path.ends_with(".target")
}

fn warn_type_conflict(key_path: &str, base_type: &str, overlay_type: &str) {
    let location = if key_path.is_empty() {
        "<root>".to_string()
    } else {
        key_path.to_string()
    };
    eprintln!(
        "warning: config key `{location}` has incompatible types (defaults: {base_type}, override: {overlay_type}); keeping default"
    );
}

fn describe_value(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Sequence(_) => "sequence",
        Value::Mapping(_) => "mapping",
        Value::Tagged(_) => "tagged",
    }
}
