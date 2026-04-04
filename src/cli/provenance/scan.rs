use commands::manifest;
use commands::manifest::provenance::read as read_sidecar;
use console::Style;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn print_summary(directory: &Path, source_filter: Option<&str>, show_orphans: bool) -> i32 {
    let mut by_source: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut orphans: Vec<String> = Vec::new();

    for kind in commands::provider::ContentKind::ALL {
        let kind_directory = directory.join(kind.as_str());
        if kind_directory.is_dir() {
            collect_recursive(&kind_directory, directory, &mut by_source, &mut orphans);
        }
    }

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

    let has_problems =
        !orphans.is_empty() || by_source.values().any(|(verified, total)| verified != total);
    i32::from(show_orphans && has_problems)
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

        if path.extension().unwrap_or_default() != "md" {
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
