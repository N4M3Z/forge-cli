use serde::Deserialize;

/// Typed representation of an in-toto/SLSA v1.0 provenance statement.
///
/// Deserializes the `.yaml` sidecar format used by forge-cli:
///
/// ```yaml
/// provenance:
///     _type: https://in-toto.io/Statement/v1
///     subject:
///         - name: claude/agents/GameMaster.md
///           digest:
///               sha256: abc123...
///     predicate:
///         buildDefinition:
///             externalParameters:
///                 source: https://github.com/N4M3Z/forge-gm
///             resolvedDependencies:
///                 - uri: agents/GameMaster.md
///                   digest:
///                       sha256: def456...
///         runDetails:
///             builder:
///                 id: forge-cli
///                 version:
///                     forge: 0.1.0
///             metadata:
///                 sourceModule: forge-gm
///                 startedOn: "2026-03-29T10:00:00Z"
/// ```
#[derive(Debug, Deserialize)]
pub struct ProvenanceSidecar {
    pub provenance: ProvenanceStatement,
}

#[derive(Debug, Deserialize)]
pub struct ProvenanceStatement {
    #[serde(rename = "_type")]
    pub statement_type: String,
    pub subject: Vec<Subject>,
    pub predicate: Predicate,
}

#[derive(Debug, Deserialize)]
pub struct Subject {
    pub name: String,
    pub digest: DigestMap,
}

#[derive(Debug, Deserialize)]
pub struct DigestMap {
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Predicate {
    pub build_definition: BuildDefinition,
    pub run_details: RunDetails,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildDefinition {
    pub build_type: String,
    pub external_parameters: ExternalParameters,
    pub resolved_dependencies: Vec<Dependency>,
}

#[derive(Debug, Deserialize)]
pub struct ExternalParameters {
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct Dependency {
    pub uri: String,
    pub digest: DigestMap,
}

#[derive(Debug, Deserialize)]
pub struct RunDetails {
    pub builder: Builder,
    pub metadata: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct Builder {
    pub id: String,
    pub version: BuilderVersion,
}

#[derive(Debug, Deserialize)]
pub struct BuilderVersion {
    pub forge: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    pub started_on: String,
}

/// Parse a provenance sidecar from YAML content.
pub fn parse(content: &str) -> Result<ProvenanceSidecar, String> {
    serde_yaml::from_str(content)
        .map_err(|error| format!("invalid provenance YAML: {error}"))
}

/// Read and parse a provenance sidecar from a file path.
pub fn read(sidecar_path: &std::path::Path) -> Result<ProvenanceSidecar, String> {
    let content = std::fs::read_to_string(sidecar_path)
        .map_err(|error| format!("cannot read {}: {error}", sidecar_path.display()))?;
    parse(&content)
}
