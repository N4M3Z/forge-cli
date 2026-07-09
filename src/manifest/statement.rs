use crate::manifest::provenance::{
    BuildDefinition, Builder, BuilderVersion, Dependency, DigestMap, ExternalParameters, Metadata,
    Predicate, ProvenanceSidecar, ProvenanceStatement, RunDetails, STATEMENT_TYPE, Subject,
};

pub fn generate_statement(
    subject_name: &str,
    subject_digest: &str,
    inputs: &[(String, String)],
    builder_id: &str,
    build_type: &str,
    builder_version: &str,
    source_uri: &str,
) -> String {
    let sidecar = ProvenanceSidecar {
        provenance: ProvenanceStatement {
            statement_type: STATEMENT_TYPE.to_string(),
            subject: vec![Subject {
                name: subject_name.to_string(),
                digest: DigestMap {
                    sha256: subject_digest.to_string(),
                },
            }],
            predicate: Predicate {
                build_definition: BuildDefinition {
                    build_type: build_type.to_string(),
                    external_parameters: ExternalParameters {
                        source: source_uri.to_string(),
                        ..ExternalParameters::default()
                    },
                    resolved_dependencies: inputs
                        .iter()
                        .map(|(uri, digest)| Dependency {
                            uri: uri.clone(),
                            digest: DigestMap {
                                sha256: digest.clone(),
                            },
                            ..Dependency::default()
                        })
                        .collect(),
                },
                run_details: RunDetails {
                    builder: Builder {
                        id: builder_id.to_string(),
                        version: BuilderVersion {
                            forge: builder_version.to_string(),
                        },
                    },
                    metadata: Metadata {
                        started_on: chrono::Utc::now().to_rfc3339(),
                    },
                },
            },
        },
    };
    serde_yaml::to_string(&sidecar)
        .expect("ProvenanceSidecar serialization is infallible by construction")
}

pub fn generate_adopt_statement(
    subject_name: &str,
    subject_digest: &str,
    upstream_url: &str,
    upstream_commit: &str,
    upstream_digest: &str,
) -> String {
    let sidecar = ProvenanceSidecar {
        provenance: ProvenanceStatement {
            statement_type: STATEMENT_TYPE.to_string(),
            subject: vec![Subject {
                name: subject_name.to_string(),
                digest: DigestMap {
                    sha256: subject_digest.to_string(),
                },
            }],
            predicate: Predicate {
                build_definition: BuildDefinition {
                    build_type: "adopt/v1".to_string(),
                    external_parameters: ExternalParameters {
                        upstream_url: upstream_url.to_string(),
                        upstream_commit: Some(upstream_commit.to_string()),
                        transforms_applied: vec!["align".to_string()],
                        ..ExternalParameters::default()
                    },
                    resolved_dependencies: vec![Dependency {
                        name: "upstream".to_string(),
                        uri: upstream_url.to_string(),
                        digest: DigestMap {
                            sha256: upstream_digest.to_string(),
                        },
                    }],
                },
                run_details: RunDetails {
                    builder: Builder {
                        id: "forge-cli".to_string(),
                        version: BuilderVersion {
                            forge: env!("CARGO_PKG_VERSION").to_string(),
                        },
                    },
                    metadata: Metadata {
                        started_on: chrono::Utc::now().to_rfc3339(),
                    },
                },
            },
        },
    };
    serde_yaml::to_string(&sidecar)
        .expect("ProvenanceSidecar serialization is infallible by construction")
}
