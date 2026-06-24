use serde::{Deserialize, Serialize};

pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";

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
#[derive(Debug, Deserialize, Serialize)]
pub struct ProvenanceSidecar {
    pub provenance: ProvenanceStatement,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ProvenanceStatement {
    #[serde(rename = "_type")]
    pub statement_type: String,
    pub subject: Vec<Subject>,
    pub predicate: Predicate,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Subject {
    pub name: String,
    pub digest: DigestMap,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct DigestMap {
    pub sha256: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Predicate {
    #[serde(default)]
    pub build_definition: BuildDefinition,
    #[serde(default)]
    pub run_details: RunDetails,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildDefinition {
    #[serde(default)]
    pub build_type: String,
    #[serde(default)]
    pub external_parameters: ExternalParameters,
    #[serde(default)]
    pub resolved_dependencies: Vec<Dependency>,
}

impl BuildDefinition {
    /// The provenance source URI, tolerant of both schemas: `assemble/v1` and
    /// `copy/v1` carry `externalParameters.source`; `adopt/v1` carries
    /// `externalParameters.upstream_url`. Returns whichever is populated.
    #[must_use]
    pub fn resolved_source(&self) -> &str {
        if self.external_parameters.source.is_empty() {
            &self.external_parameters.upstream_url
        } else {
            &self.external_parameters.source
        }
    }
}

/// External build parameters. `source` is the forge-side origin URI;
/// `upstream_url` / `upstream_commit` / `transforms_applied` appear instead on
/// `adopt/v1` sidecars. All but `source` are skipped on serialization when
/// empty so generated `assemble/v1` sidecars are unchanged.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct ExternalParameters {
    #[serde(default)]
    pub source: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub upstream_url: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub upstream_commit: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub transforms_applied: Vec<String>,
}

/// A resolved dependency. `name` is present on `adopt/v1` sidecars (e.g.
/// `upstream`, or a transform-skill name) and skipped on serialization when
/// empty so generated sidecars without names are unchanged.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Dependency {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(default)]
    pub uri: String,
    #[serde(default)]
    pub digest: DigestMap,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct RunDetails {
    #[serde(default)]
    pub builder: Builder,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Builder {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub version: BuilderVersion,
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct BuilderVersion {
    #[serde(default)]
    pub forge: String,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Metadata {
    #[serde(default)]
    pub started_on: String,
}

/// Parse a provenance sidecar from YAML content.
pub fn parse(content: &str) -> Result<ProvenanceSidecar, String> {
    serde_yaml::from_str(content).map_err(|error| format!("invalid provenance YAML: {error}"))
}

/// Read and parse a provenance sidecar from a file path.
pub fn read(sidecar_path: &std::path::Path) -> Result<ProvenanceSidecar, String> {
    let content = std::fs::read_to_string(sidecar_path)
        .map_err(|error| format!("cannot read {}: {error}", sidecar_path.display()))?;
    parse(&content)
}
