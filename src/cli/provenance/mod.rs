use commands::error::{Error, ErrorKind};
use commands::manifest;
use commands::manifest::provenance::read as read_sidecar;
use console::Style;
use std::fs;
use std::path::Path;

mod scan;

pub fn execute(
    path: &str,
    source_filter: Option<&str>,
    show_orphans: bool,
    _json_output: bool,
) -> Result<i32, Error> {
    let target = Path::new(path);

    if target.is_dir() {
        return Ok(scan::print_summary(target, source_filter, show_orphans));
    }

    let file_path = target;
    if !file_path.is_file() {
        return Err(Error::new(
            ErrorKind::Io,
            format!("{} is not a file or directory", file_path.display()),
        ));
    }

    let sidecar = read_sidecar(&resolve_sidecar_path(file_path))
        .map_err(|error| Error::new(ErrorKind::Io, format!("no provenance found: {error}")))?;

    let statement = &sidecar.provenance;
    let definition = &statement.predicate.build_definition;
    let details = &statement.predicate.run_details;
    let output_hash = &statement.subject[0].digest.sha256;

    let deployed_content = fs::read_to_string(file_path).unwrap_or_default();
    let deployed_hash = manifest::content_sha256(&deployed_content);
    let chain_verified = deployed_hash == *output_hash;

    let green = Style::new().green();
    let red = Style::new().red();
    let dim = Style::new().dim();
    let bold = Style::new().bold();
    let cyan = Style::new().cyan();

    let display_path = file_path.to_string_lossy();

    println!();
    println!(" {} {}", dim.apply_to("┌"), bold.apply_to(&*display_path));
    println!(" {}", dim.apply_to("│"));
    println!(
        " {}  {}   {}",
        dim.apply_to("│"),
        dim.apply_to("source"),
        cyan.apply_to(&definition.external_parameters.source)
    );
    println!(
        " {}  {}    {}",
        dim.apply_to("│"),
        dim.apply_to("built"),
        &details.metadata.started_on
    );
    println!(
        " {}  {}  {} {}",
        dim.apply_to("│"),
        dim.apply_to("builder"),
        &details.builder.id,
        dim.apply_to(&details.builder.version.forge)
    );
    println!(" {}", dim.apply_to("│"));

    for dependency in &definition.resolved_dependencies {
        let short = &dependency.digest.sha256[..16.min(dependency.digest.sha256.len())];
        println!(
            " {}  {} {} {}",
            dim.apply_to("│"),
            dim.apply_to("input"),
            &dependency.uri,
            dim.apply_to(format!("sha256:{short}"))
        );
    }

    let short_output = &output_hash[..16.min(output_hash.len())];
    println!(
        " {}  {} {}",
        dim.apply_to("│"),
        dim.apply_to("output"),
        dim.apply_to(format!("sha256:{short_output}"))
    );
    println!(" {}", dim.apply_to("│"));

    if chain_verified {
        println!(
            " {}  {} {}",
            dim.apply_to("│"),
            green.apply_to("✓"),
            green.apply_to("hash verified")
        );
    } else {
        println!(
            " {}  {} {}",
            dim.apply_to("│"),
            red.apply_to("✗"),
            red.apply_to("hash mismatch — file modified after deployment")
        );
    }

    println!(" {}", dim.apply_to("└"));
    println!();

    Ok(0)
}

/// Resolve a deployed file's provenance sidecar path.
///
/// Two naming conventions coexist across forge modules:
///
/// | Convention | Example sidecar |
/// |---|---|
/// | Extension-less | `agents/.provenance/CodeReviewer.yaml` (for `CodeReviewer.md`) |
/// | Extension-preserving | `skills/.provenance/CleanCode.md.yaml` (for `CleanCode.md`) |
///
/// Returns the first path that exists on disk; falls back to the
/// extension-less form when neither exists (for clearer error messages).
pub(crate) fn resolve_sidecar_path(file_path: &Path) -> std::path::PathBuf {
    let parent = file_path.parent().unwrap_or(Path::new("."));
    let provenance_directory = parent.join(manifest::PROVENANCE_DIRECTORY);

    let stem = file_path.file_stem().unwrap_or_default().to_string_lossy();
    let stem_path = provenance_directory.join(format!("{stem}.{}", manifest::SIDECAR_EXTENSION));

    let basename = file_path.file_name().unwrap_or_default().to_string_lossy();
    let extension_preserving_path =
        provenance_directory.join(format!("{basename}.{}", manifest::SIDECAR_EXTENSION));

    if stem_path.is_file() {
        return stem_path;
    }
    if extension_preserving_path.is_file() {
        return extension_preserving_path;
    }
    stem_path
}

#[cfg(test)]
mod tests;
