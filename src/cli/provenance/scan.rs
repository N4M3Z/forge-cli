use commands::manifest;
use commands::manifest::provenance::read as read_sidecar;
use console::Style;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

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

        // Walk every deployed content file regardless of extension. Codex
        // assembly turns agent .md files into .toml; previously only .md
        // was inspected, so codex sidecars were never matched against
        // their files (issue #29). Skip our own .yaml sidecars and any
        // dotfile (e.g. .DS_Store, .manifest).
        let basename = path.file_name().unwrap_or_default().to_string_lossy();
        if basename.starts_with('.') {
            continue;
        }
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
        std::fs::write(target.path().join(".manifest"), "").unwrap();

        let (by_source, orphans) = collect(target.path());

        assert!(
            by_source.is_empty(),
            ".DS_Store and .manifest must not appear"
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
}
