use commands::error::{Error, ErrorKind};
use commands::manifest;
use std::fs;
use std::path::Path;

use super::sources::SourceFile;

/// Build an in-toto/SLSA provenance statement for a single assembled file.
pub fn build_statement(manifest_key: &str, assembled: &str, source: &SourceFile) -> String {
    let output_sha256 = manifest::content_sha256(assembled);
    let source_sha256 = manifest::content_sha256(&source.content);

    manifest::generate_statement(
        manifest_key,
        &output_sha256,
        &[(source.relative_path.clone(), source_sha256)],
        "forge-cli",
        "https://forge.example/build/v1",
        env!("CARGO_PKG_VERSION"),
    )
}

/// Write a `.yaml` sidecar file next to the assembled output.
pub fn write_sidecar(output_path: &Path, statement: &str) -> Result<(), Error> {
    let prov_path = output_path.with_extension("yaml");
    fs::write(&prov_path, statement).map_err(|e| {
        Error::new(
            ErrorKind::Io,
            format!("cannot write {}: {e}", prov_path.display()),
        )
    })
}
