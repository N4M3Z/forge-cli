const STATEMENT_TEMPLATE: &str = include_str!("statement.yaml");
const DEPENDENCY_TEMPLATE: &str = include_str!("dependency.yaml");

pub fn generate_statement(
    subject_name: &str,
    subject_digest: &str,
    inputs: &[(String, String)],
    builder_id: &str,
    build_type: &str,
    builder_version: &str,
    source_uri: &str,
) -> String {
    let timestamp = chrono::Utc::now().to_rfc3339();

    let mut dependencies = String::new();
    for (uri, digest) in inputs {
        let entry = DEPENDENCY_TEMPLATE
            .replace("{uri}", uri)
            .replace("{digest}", digest);
        dependencies.push_str(&entry);
    }

    STATEMENT_TEMPLATE
        .replace("{subject_name}", subject_name)
        .replace("{subject_digest}", subject_digest)
        .replace("{resolved_dependencies}\n", &dependencies)
        .replace("{build_type}", build_type)
        .replace("{builder_id}", builder_id)
        .replace("{builder_version}", builder_version)
        .replace("{source_uri}", source_uri)
        .replace("{timestamp}", &timestamp)
}
