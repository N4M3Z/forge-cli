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
                    },
                    resolved_dependencies: inputs
                        .iter()
                        .map(|(uri, digest)| Dependency {
                            uri: uri.clone(),
                            digest: DigestMap {
                                sha256: digest.clone(),
                            },
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
