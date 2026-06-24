use commands::manifest;
use commands::manifest::provenance::read as read_sidecar;
use console::Style;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn print_summary(directory: &Path, source_filter: Option<&str>, show_orphans: bool) -> i32 {
    let (mut by_source, orphans) = collect(directory);

    if let Some(filter) = source_filter {
        by_source.retain(|source, _| source.contains(filter));
    }

    let green = Style::new().green();
    let red = Style::new().red();
    let dim = Style::new().dim();
    let bold = Style::new().bold();

    if by_source.is_empty() && orphans.is_empty() {
        println!("\n No provenance found in {}\n", directory.display());
        return 0;
    }

    println!();
    for (source_uri, (verified_count, total_count)) in &by_source {
        let status = if verified_count == total_count {
            green.apply_to(format!("✓ {total_count} verified"))
        } else {
            red.apply_to(format!("✗ {verified_count}/{total_count} verified"))
        };
        println!(
            " {} {} {}",
            bold.apply_to(source_uri),
            dim.apply_to("→"),
            status
        );
    }

    if show_orphans && !orphans.is_empty() {
        println!();
        println!(
            " {} {}",
            red.apply_to("orphans"),
            dim.apply_to(format!("({} files without provenance)", orphans.len()))
        );
        for orphan in &orphans {
            println!("   {} {orphan}", red.apply_to("•"));
        }
    }

    println!();

    let has_problems = !orphans.is_empty()
        || by_source
            .values()
            .any(|(verified, total)| verified != total);
    i32::from(show_orphans && has_problems)
}

/// Walk the deployed content kinds under `directory`, returning per-source
/// verification counts and the list of files without a matching sidecar.
fn collect(directory: &Path) -> (BTreeMap<String, (usize, usize)>, Vec<String>) {
    let mut by_source: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut orphans: Vec<String> = Vec::new();

    for kind in commands::provider::ContentKind::ALL {
        let kind_directory = directory.join(kind.as_str());
        if kind_directory.is_dir() {
            collect_recursive(&kind_directory, directory, &mut by_source, &mut orphans);
        }
    }

    (by_source, orphans)
}

/// Files in the same category directory (`agents/`, `rules/`, ...) must
/// have unique names ignoring extension. If `Foo.md` and `Foo.toml` ever
/// land side by side, both look up the same sidecar at
/// `.provenance/Foo.yaml` and one will be reported as a digest mismatch
/// with no name attached. Co-installed providers should deploy into
/// separate targets.
fn collect_recursive(
    directory: &Path,
    target_root: &Path,
    by_source: &mut BTreeMap<String, (usize, usize)>,
    orphans: &mut Vec<String>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path.is_dir() {
            let dirname = path.file_name().unwrap_or_default().to_string_lossy();
            if !dirname.starts_with('.') {
                collect_recursive(&path, target_root, by_source, orphans);
            }
            continue;
        }

        let basename = path.file_name().unwrap_or_default().to_string_lossy();
        if basename.starts_with('.') {
            continue;
        }
        // Defensive: sidecars live under `.provenance/`, which the
        // directory walk already skips. This guards against future raw
        // `.yaml` content placed alongside `.md`/`.toml`.
        if path.extension().unwrap_or_default() == manifest::SIDECAR_EXTENSION {
            continue;
        }

        let relative = path
            .strip_prefix(target_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let Ok(sidecar) = read_sidecar(&super::resolve_sidecar_path(&path)) else {
            orphans.push(relative);
            continue;
        };

        let source = sidecar
            .provenance
            .predicate
            .build_definition
            .external_parameters
            .source
            .clone();

        let output_hash = &sidecar.provenance.subject[0].digest.sha256;
        let deployed_content = fs::read_to_string(&path).unwrap_or_default();
        let deployed_hash = manifest::content_sha256(&deployed_content);

        let counts = by_source.entry(source).or_insert((0, 0));
        counts.1 += 1;
        if deployed_hash == *output_hash {
            counts.0 += 1;
        }
    }
}

// --- Source-side verification ---
//
// A source repository (one with a `module.yaml`) carries `.provenance/*.yaml`
// sidecars next to the artifacts they describe. Unlike a deployed target —
// where the sidecar is resolved FROM a deployed file — source verification
// walks the sidecars and resolves each `subject.name` back to a repo-relative
// file, recomputing its SHA-256 and that of any in-repo `resolvedDependencies`.

#[derive(Serialize)]
struct SidecarReport {
    subject: String,
    build_type: String,
    source: String,
    verified: bool,
    expected_sha256: String,
    actual_sha256: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dependencies: Vec<DependencyReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct DependencyReport {
    name: String,
    uri: String,
    verified: bool,
    expected_sha256: String,
    actual_sha256: String,
}

impl SidecarReport {
    fn is_clean(&self) -> bool {
        self.error.is_none() && self.verified && self.dependencies.iter().all(|dep| dep.verified)
    }
}

/// Verify source-side provenance under `scan_dir`, resolving subjects against
/// `module_root`. Returns a process exit code (nonzero when any sidecar is
/// stale, dangling, or unparseable).
pub fn print_source_summary(
    module_root: &Path,
    scan_dir: &Path,
    source_filter: Option<&str>,
    json_output: bool,
) -> i32 {
    let mut reports: Vec<SidecarReport> = Vec::new();
    collect_source_sidecars(module_root, scan_dir, &mut reports);
    reports.sort_by(|left, right| left.subject.cmp(&right.subject));

    if let Some(filter) = source_filter {
        reports.retain(|report| report.source.contains(filter) || report.subject.contains(filter));
    }

    if json_output {
        print_source_json(&reports);
    } else {
        print_source_console(scan_dir, &reports);
    }

    i32::from(reports.iter().any(|report| !report.is_clean()))
}

/// Verify a single source artifact by resolving its `.provenance/<stem>.yaml`
/// sidecar. Returns a nonzero exit code when stale, dangling, or unparseable.
pub fn print_source_file(module_root: &Path, file_path: &Path, json_output: bool) -> i32 {
    let sidecar_path = super::resolve_sidecar_path(file_path);
    if !sidecar_path.is_file() {
        println!(
            "\n No provenance sidecar for {} (expected {})\n",
            file_path.display(),
            sidecar_path.display()
        );
        return 1;
    }
    let report = verify_sidecar(module_root, &sidecar_path);
    let clean = report.is_clean();
    if json_output {
        print_source_json(std::slice::from_ref(&report));
    } else {
        print_source_console(file_path, std::slice::from_ref(&report));
    }
    i32::from(!clean)
}

fn collect_source_sidecars(module_root: &Path, dir: &Path, reports: &mut Vec<SidecarReport>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name == manifest::PROVENANCE_DIRECTORY {
            collect_from_provenance_dir(module_root, &path, reports);
        } else if !name.starts_with('.') {
            collect_source_sidecars(module_root, &path, reports);
        }
    }
}

fn collect_from_provenance_dir(
    module_root: &Path,
    provenance_dir: &Path,
    reports: &mut Vec<SidecarReport>,
) {
    let Ok(entries) = fs::read_dir(provenance_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().unwrap_or_default() == manifest::SIDECAR_EXTENSION {
            reports.push(verify_sidecar(module_root, &path));
        }
    }
}

fn verify_sidecar(module_root: &Path, sidecar_path: &Path) -> SidecarReport {
    let label = sidecar_path.to_string_lossy().to_string();
    let sidecar = match read_sidecar(sidecar_path) {
        Ok(sidecar) => sidecar,
        Err(parse_error) => return error_report(label, parse_error),
    };

    let statement = &sidecar.provenance;
    let Some(subject) = statement.subject.first() else {
        return error_report(label, "sidecar has no subject".to_string());
    };
    let definition = &statement.predicate.build_definition;

    let (verified, actual) = match resolve_in_repo(module_root, &subject.name) {
        Some(resolved) => {
            let hash = manifest::content_sha256(&fs::read_to_string(&resolved).unwrap_or_default());
            (hash == subject.digest.sha256, hash)
        }
        None => (false, String::new()),
    };

    let dependencies = definition
        .resolved_dependencies
        .iter()
        .filter_map(|dependency| verify_dependency(module_root, dependency))
        .collect();

    SidecarReport {
        subject: subject.name.clone(),
        build_type: definition.build_type.clone(),
        source: definition.resolved_source().to_string(),
        verified,
        expected_sha256: subject.digest.sha256.clone(),
        actual_sha256: actual,
        dependencies,
        error: None,
    }
}

fn verify_dependency(
    module_root: &Path,
    dependency: &commands::manifest::provenance::Dependency,
) -> Option<DependencyReport> {
    // Remote upstream dependencies cannot be verified offline; the sidecar's
    // recorded digest is the pin. Only in-repo dependencies are recomputed.
    if dependency.name == "upstream" || is_remote_uri(&dependency.uri) {
        return None;
    }
    let resolved = resolve_in_repo_dependency(module_root, &dependency.uri)?;
    let hash = manifest::content_sha256(&fs::read_to_string(&resolved).unwrap_or_default());
    Some(DependencyReport {
        name: dependency.name.clone(),
        uri: dependency.uri.clone(),
        verified: hash == dependency.digest.sha256,
        expected_sha256: dependency.digest.sha256.clone(),
        actual_sha256: hash,
    })
}

fn error_report(label: String, message: String) -> SidecarReport {
    SidecarReport {
        subject: label,
        build_type: String::new(),
        source: String::new(),
        verified: false,
        expected_sha256: String::new(),
        actual_sha256: String::new(),
        dependencies: Vec::new(),
        error: Some(message),
    }
}

fn is_remote_uri(uri: &str) -> bool {
    uri.starts_with("http://") || uri.starts_with("https://")
}

/// Resolve a repo-relative subject name against the module root, rejecting any
/// path that escapes the root (path-boundary validation). Returns `None` when
/// the file is missing or outside the repo.
fn resolve_in_repo(module_root: &Path, repo_relative: &str) -> Option<PathBuf> {
    let candidate = module_root.join(repo_relative);
    let canonical = fs::canonicalize(&candidate).ok()?;
    let root_canonical = fs::canonicalize(module_root).ok()?;
    canonical.starts_with(&root_canonical).then_some(canonical)
}

/// Resolve an in-repo dependency URI (typically `<module-name>/<path>`) under
/// the module root. Tries the path verbatim, then with its leading segment
/// (the module name) stripped, so `forge-core/skills/X/SKILL.md` resolves.
fn resolve_in_repo_dependency(module_root: &Path, uri: &str) -> Option<PathBuf> {
    if let Some(resolved) = resolve_in_repo(module_root, uri) {
        return Some(resolved);
    }
    let without_leading_segment = uri.split_once('/').map(|(_, rest)| rest)?;
    resolve_in_repo(module_root, without_leading_segment)
}

fn print_source_json(reports: &[SidecarReport]) {
    match serde_json::to_string_pretty(reports) {
        Ok(json) => println!("{json}"),
        Err(error) => eprintln!("failed to serialize provenance report: {error}"),
    }
}

fn print_source_console(scan_dir: &Path, reports: &[SidecarReport]) {
    let green = Style::new().green();
    let red = Style::new().red();
    let dim = Style::new().dim();
    let bold = Style::new().bold();

    if reports.is_empty() {
        println!(
            "\n No source-side provenance found in {}\n",
            scan_dir.display()
        );
        return;
    }

    println!();
    for report in reports {
        if let Some(error) = &report.error {
            println!(
                " {} {} {}",
                red.apply_to("✗"),
                bold.apply_to(&report.subject),
                dim.apply_to(error)
            );
            continue;
        }

        let mark = if report.verified {
            green.apply_to("✓")
        } else if report.actual_sha256.is_empty() {
            red.apply_to("⦰")
        } else {
            red.apply_to("✗")
        };
        let status = if report.verified {
            green.apply_to("match".to_string())
        } else if report.actual_sha256.is_empty() {
            red.apply_to("missing source file".to_string())
        } else {
            red.apply_to("stale (content changed since adoption)".to_string())
        };
        println!(
            " {} {} {} {}",
            mark,
            bold.apply_to(&report.subject),
            dim.apply_to("→"),
            status
        );

        for dependency in &report.dependencies {
            let dep_mark = if dependency.verified {
                green.apply_to("✓")
            } else {
                red.apply_to("✗")
            };
            println!(
                "     {} {} {} {}",
                dep_mark,
                dim.apply_to("dep"),
                dependency.name,
                dim.apply_to(if dependency.verified {
                    "match"
                } else {
                    "stale"
                })
            );
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_sidecar_for(
        target_root: &Path,
        kind: &str,
        deployed_filename: &str,
        sidecar_subject: &str,
        digest: &str,
        source_uri: &str,
    ) {
        let kind_dir = target_root.join(kind);
        let provenance_dir = kind_dir.join(".provenance");
        std::fs::create_dir_all(&provenance_dir).unwrap();
        let stem = std::path::Path::new(deployed_filename)
            .file_stem()
            .unwrap()
            .to_string_lossy();
        let yaml = format!(
            "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: {sidecar_subject}\n          digest:\n              sha256: {digest}\n    predicate:\n        buildDefinition:\n            buildType: https://example.test/copy/v1\n            externalParameters:\n                source: {source_uri}\n            resolvedDependencies:\n                - uri: {sidecar_subject}\n                  digest:\n                      sha256: {digest}\n        runDetails:\n            builder:\n                id: forge-cli\n                version:\n                    forge: 0.0.0-test\n            metadata:\n                startedOn: \"2026-01-01T00:00:00Z\"\n"
        );
        std::fs::write(provenance_dir.join(format!("{stem}.yaml")), yaml).unwrap();
    }

    #[test]
    fn collect_walks_toml_files_for_codex_provider() {
        let target = TempDir::new().unwrap();
        let agents_dir = target.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        let toml_content = "name = \"GameMaster\"\n";
        std::fs::write(agents_dir.join("GameMaster.toml"), toml_content).unwrap();
        let digest = manifest::content_sha256(toml_content);
        write_sidecar_for(
            target.path(),
            "agents",
            "GameMaster.toml",
            "codex/agents/GameMaster.toml",
            &digest,
            "https://example.test/upstream",
        );

        let (by_source, orphans) = collect(target.path());

        assert!(
            orphans.is_empty(),
            "toml file with sidecar must not be orphaned"
        );
        assert_eq!(by_source.len(), 1, "one source bucket expected");
        let (verified, total) = by_source["https://example.test/upstream"];
        assert_eq!(total, 1, "the toml file should be counted");
        assert_eq!(verified, 1, "matching sha256 should verify");
    }

    #[test]
    fn collect_still_walks_md_files_for_other_providers() {
        let target = TempDir::new().unwrap();
        let agents_dir = target.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        let md_content = "# Agent body\n";
        std::fs::write(agents_dir.join("ClaudeAgent.md"), md_content).unwrap();
        let digest = manifest::content_sha256(md_content);
        write_sidecar_for(
            target.path(),
            "agents",
            "ClaudeAgent.md",
            "claude/agents/ClaudeAgent.md",
            &digest,
            "https://example.test/upstream",
        );

        let (by_source, orphans) = collect(target.path());

        assert!(orphans.is_empty());
        let (verified, total) = by_source["https://example.test/upstream"];
        assert_eq!(total, 1);
        assert_eq!(verified, 1);
    }

    #[test]
    fn collect_skips_dotfiles_and_sidecars() {
        let target = TempDir::new().unwrap();
        let agents_dir = target.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join(".DS_Store"), b"").unwrap();
        std::fs::write(agents_dir.join("stray.yaml"), b"").unwrap();

        let (by_source, orphans) = collect(target.path());

        assert!(
            by_source.is_empty(),
            "dotfile and stray .yaml must not appear"
        );
        assert!(orphans.is_empty());
    }

    #[test]
    fn collect_reports_files_without_sidecars_as_orphans() {
        let target = TempDir::new().unwrap();
        let agents_dir = target.path().join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("Lonely.md"), "no sidecar").unwrap();

        let (by_source, orphans) = collect(target.path());

        assert!(by_source.is_empty());
        assert_eq!(orphans, vec!["agents/Lonely.md".to_string()]);
    }

    // --- source-side verification ---

    fn write_adopt_sidecar(
        root: &Path,
        artifact_relative: &str,
        subject_digest: &str,
        dep: Option<(&str, &str, &str)>,
    ) {
        let artifact = Path::new(artifact_relative);
        let artifact_dir = root.join(artifact.parent().unwrap());
        let provenance_dir = artifact_dir.join(".provenance");
        std::fs::create_dir_all(&provenance_dir).unwrap();
        let stem = artifact.file_stem().unwrap().to_string_lossy();
        let dep_block = match dep {
            Some((name, uri, digest)) => format!(
                "                - name: {name}\n                  uri: {uri}\n                  digest:\n                      sha256: {digest}\n"
            ),
            None => String::new(),
        };
        let yaml = format!(
            "provenance:\n    _type: https://in-toto.io/Statement/v1\n    subject:\n        - name: {artifact_relative}\n          digest:\n              sha256: {subject_digest}\n    predicate:\n        buildDefinition:\n            buildType: https://forge-cli/adopt/v1\n            externalParameters:\n                upstream_url: https://example.test/upstream\n            resolvedDependencies:\n                - name: upstream\n                  uri: https://example.test/upstream\n                  digest:\n                      sha256: deadbeef\n{dep_block}"
        );
        std::fs::write(provenance_dir.join(format!("{stem}.yaml")), yaml).unwrap();
    }

    fn source_repo() -> TempDir {
        let repo = TempDir::new().unwrap();
        std::fs::write(repo.path().join("module.yaml"), "name: fixture\n").unwrap();
        repo
    }

    #[test]
    fn source_verify_matches_when_unchanged() {
        let repo = source_repo();
        let body = "Adopted skill body.\n";
        let skill_dir = repo.path().join("skills/Adopted");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), body).unwrap();
        write_adopt_sidecar(
            repo.path(),
            "skills/Adopted/SKILL.md",
            &manifest::content_sha256(body),
            None,
        );

        let mut reports = Vec::new();
        collect_source_sidecars(repo.path(), repo.path(), &mut reports);
        assert_eq!(reports.len(), 1, "one sidecar verified, not orphaned");
        assert!(reports[0].verified, "subject digest must match");
        assert!(reports[0].error.is_none());
    }

    #[test]
    fn source_verify_flags_stale_subject() {
        let repo = source_repo();
        let skill_dir = repo.path().join("skills/Adopted");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "edited after adoption\n").unwrap();
        write_adopt_sidecar(
            repo.path(),
            "skills/Adopted/SKILL.md",
            &manifest::content_sha256("original body\n"),
            None,
        );

        let mut reports = Vec::new();
        collect_source_sidecars(repo.path(), repo.path(), &mut reports);
        assert!(!reports[0].verified, "edited subject must be stale");
        assert_eq!(
            reports[0].actual_sha256,
            manifest::content_sha256("edited after adoption\n")
        );
    }

    #[test]
    fn source_verify_checks_in_repo_dependency() {
        let repo = source_repo();
        let skill_dir = repo.path().join("skills/Adopted");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let body = "body\n";
        std::fs::write(skill_dir.join("SKILL.md"), body).unwrap();

        // In-repo transform-skill dependency, referenced as fixture/<path>.
        let transform_dir = repo.path().join("skills/AdoptArtifact");
        std::fs::create_dir_all(&transform_dir).unwrap();
        let transform_body = "transform skill\n";
        std::fs::write(transform_dir.join("SKILL.md"), transform_body).unwrap();

        write_adopt_sidecar(
            repo.path(),
            "skills/Adopted/SKILL.md",
            &manifest::content_sha256(body),
            Some((
                "AdoptArtifact",
                "fixture/skills/AdoptArtifact/SKILL.md",
                &manifest::content_sha256(transform_body),
            )),
        );

        let mut reports = Vec::new();
        collect_source_sidecars(repo.path(), repo.path(), &mut reports);
        assert_eq!(reports[0].dependencies.len(), 1, "upstream dep is skipped");
        assert!(
            reports[0].dependencies[0].verified,
            "in-repo dep digest must match"
        );
    }

    #[test]
    fn source_verify_flags_stale_in_repo_dependency() {
        let repo = source_repo();
        let skill_dir = repo.path().join("skills/Adopted");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let body = "body\n";
        std::fs::write(skill_dir.join("SKILL.md"), body).unwrap();
        let transform_dir = repo.path().join("skills/AdoptArtifact");
        std::fs::create_dir_all(&transform_dir).unwrap();
        std::fs::write(transform_dir.join("SKILL.md"), "transform changed\n").unwrap();

        write_adopt_sidecar(
            repo.path(),
            "skills/Adopted/SKILL.md",
            &manifest::content_sha256(body),
            Some((
                "AdoptArtifact",
                "fixture/skills/AdoptArtifact/SKILL.md",
                &manifest::content_sha256("original transform\n"),
            )),
        );

        let mut reports = Vec::new();
        collect_source_sidecars(repo.path(), repo.path(), &mut reports);
        assert!(!reports[0].dependencies[0].verified, "dep must be stale");
        assert!(
            !reports[0].is_clean(),
            "a stale dependency makes the report unclean"
        );
    }

    #[test]
    fn source_verify_flags_missing_source_file() {
        let repo = source_repo();
        std::fs::create_dir_all(repo.path().join("skills/Ghost")).unwrap();
        write_adopt_sidecar(repo.path(), "skills/Ghost/SKILL.md", "abc123", None);

        let mut reports = Vec::new();
        collect_source_sidecars(repo.path(), repo.path(), &mut reports);
        assert!(!reports[0].verified);
        assert!(reports[0].actual_sha256.is_empty(), "dangling sidecar");
    }

    #[test]
    fn source_verify_reports_unparseable_sidecar() {
        let repo = source_repo();
        let provenance_dir = repo.path().join("skills/Bad/.provenance");
        std::fs::create_dir_all(&provenance_dir).unwrap();
        std::fs::write(provenance_dir.join("SKILL.yaml"), "not: [valid").unwrap();

        let mut reports = Vec::new();
        collect_source_sidecars(repo.path(), repo.path(), &mut reports);
        assert_eq!(reports.len(), 1);
        assert!(
            reports[0].error.is_some(),
            "parse error is reported, not silent"
        );
        assert!(!reports[0].is_clean());
    }
}
