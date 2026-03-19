pub fn string_field(
    value: &serde_yaml::Value,
    field: &str,
    entry_name: &str,
) -> Result<String, String> {
    match value.get(field) {
        Some(v) => match v.as_str() {
            Some(s) => Ok(s.to_string()),
            None => Err(format!("{field} is not a string for {entry_name}")),
        },
        None => Err(format!("missing {field} for {entry_name}")),
    }
}
