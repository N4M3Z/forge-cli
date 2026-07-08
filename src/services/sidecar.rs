//! SLSA provenance sidecar serde model and direct parsing.

use crate::view::{Adoption, Dependency};

#[derive(serde::Deserialize)]
pub(super) struct Sidecar {
    pub(super) provenance: Statement,
}
#[derive(serde::Deserialize)]
pub(super) struct Statement {
    #[serde(default)]
    pub(super) subject: Vec<SubjectRef>,
    pub(super) predicate: Predicate,
    #[serde(default)]
    pub(super) attribution: Attribution,
}
#[derive(serde::Deserialize, Default)]
pub(super) struct SubjectRef {
    #[serde(default)]
    pub(super) digest: DependencyDigest,
}
#[derive(serde::Deserialize)]
pub(super) struct Predicate {
    #[serde(rename = "buildDefinition")]
    pub(super) build_definition: BuildDefinition,
}
#[derive(serde::Deserialize)]
pub(super) struct BuildDefinition {
    #[serde(rename = "buildType", default)]
    pub(super) build_type: String,
    #[serde(rename = "externalParameters", default)]
    pub(super) external_parameters: ExternalParameters,
    #[serde(rename = "resolvedDependencies", default)]
    pub(super) resolved_dependencies: Vec<ResolvedDependency>,
}
#[derive(serde::Deserialize, Default)]
pub(super) struct ExternalParameters {
    #[serde(default)]
    pub(super) source: String,
    #[serde(default)]
    pub(super) upstream_url: String,
    #[serde(default)]
    pub(super) upstream_commit: String,
    #[serde(default)]
    pub(super) transforms_applied: Vec<String>,
}
#[derive(serde::Deserialize)]
pub(super) struct ResolvedDependency {
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) uri: String,
    #[serde(default)]
    pub(super) digest: DependencyDigest,
}
#[derive(serde::Deserialize, Default)]
pub(super) struct DependencyDigest {
    #[serde(default)]
    pub(super) sha256: String,
}
#[derive(serde::Deserialize, Default)]
pub(super) struct Attribution {
    #[serde(default)]
    pub(super) upstream_author: String,
    #[serde(default)]
    pub(super) upstream_license: String,
    #[serde(default)]
    pub(super) adopted_by: String,
}

/// Parses an `adopt/v1` or `copy/v1` provenance sidecar into a view `Adoption`.
pub(super) fn parse_adoption(content: &str) -> Option<Adoption> {
    let sidecar: Sidecar = serde_yaml::from_str(content).ok()?;
    let definition = &sidecar.provenance.predicate.build_definition;
    let params = &definition.external_parameters;
    let kind = if definition.build_type.contains("adopt") {
        "adopt"
    } else if definition.build_type.contains("copy") {
        "copy"
    } else {
        "build"
    };
    let source = if params.upstream_url.is_empty() {
        params.source.clone()
    } else {
        params.upstream_url.clone()
    };
    let source_sha = definition
        .resolved_dependencies
        .iter()
        .find(|dependency| dependency.name == "upstream")
        .map(|dependency| dependency.digest.sha256.clone())
        .unwrap_or_default();
    let dependencies = definition
        .resolved_dependencies
        .iter()
        .filter(|dependency| dependency.name != "upstream" && !dependency.name.is_empty())
        .map(|dependency| Dependency {
            name: dependency.name.clone(),
            uri: dependency.uri.clone(),
            sha: dependency.digest.sha256.clone(),
        })
        .collect();
    let (source_repo, source_label) = shorten_source(&source);
    let attribution = &sidecar.provenance.attribution;
    Some(Adoption {
        kind: kind.to_string(),
        source,
        source_repo,
        source_label,
        source_sha,
        commit: params.upstream_commit.clone(),
        transforms: params.transforms_applied.clone(),
        author: attribution.upstream_author.clone(),
        license: attribution.upstream_license.clone(),
        adopted_by: attribution.adopted_by.clone(),
        dependencies,
    })
}

/// Shortens a source URL to `(repo_url, "owner/repo")`. For a GitHub/GitLab
/// blob URL like `https://github.com/owner/repo/blob/SHA/path`, returns the
/// repo root and `owner/repo` label. Non-URL sources return `(source, source)`.
pub(super) fn shorten_source(source: &str) -> (String, String) {
    let Some(rest) = source
        .strip_prefix("https://")
        .or_else(|| source.strip_prefix("http://"))
    else {
        return (source.to_string(), source.to_string());
    };
    let segments: Vec<&str> = rest.split('/').collect();
    if segments.len() < 3 {
        return (source.to_string(), source.to_string());
    }
    let host = segments[0];
    let owner = segments[1];
    let repo = segments[2];
    let repo_url = format!("https://{host}/{owner}/{repo}");
    let label = format!("{owner}/{repo}");
    (repo_url, label)
}

/// Reads the subject digest from a sidecar (the artifact's own content hash at
/// copy time), used to detect a copy edited after adoption.
pub(super) fn recorded_subject_sha(sidecar_content: &str) -> Option<String> {
    let sidecar: Sidecar = serde_yaml::from_str(sidecar_content).ok()?;
    sidecar
        .provenance
        .subject
        .first()
        .map(|subject| subject.digest.sha256.clone())
        .filter(|sha| !sha.is_empty())
}
