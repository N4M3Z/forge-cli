use commands::assemble::strip_heading;
use commands::error::{Error, ErrorKind};
use commands::manifest::content_sha256;
use commands::parse::split_frontmatter;
use commands::provider::ContentKind;
use console::Style;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::path::Path;

const BODY_KEY: &str = "body";

// --- Types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DriftStatus {
    Identical,
    FrontmatterOnly,
    BodyOnly,
    Both,
    Expected,
    LocalOnly,
    UpstreamOnly,
}

#[derive(Debug, Serialize)]
pub struct DriftEntry {
    pub name: String,
    pub status: DriftStatus,
    pub category: String,
    pub changed_keys: Vec<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct DriftResult {
    pub entries: Vec<DriftEntry>,
    pub errors: Vec<String>,
}

// --- Execution ---

pub fn execute(
    module_path: &str,
    upstream_path: &str,
    ignore_keys: &[String],
    json_output: bool,
) -> Result<i32, Error> {
    let module_root = Path::new(module_path);
    let upstream_root = Path::new(upstream_path);

    if !module_root.is_dir() {
        return Err(Error::new(
            ErrorKind::Io,
            format!("module path is not a directory: {module_path}"),
        ));
    }
    if !upstream_root.is_dir() {
        return Err(Error::new(
            ErrorKind::Io,
            format!("upstream path is not a directory: {upstream_path}"),
        ));
    }

    let ignored: HashSet<&str> = ignore_keys.iter().map(String::as_str).collect();

    let mut result = DriftResult::default();

    for kind in ContentKind::ALL {
        compare_content_directory(&mut result, module_root, upstream_root, *kind, &ignored);
    }

    compare_decisions_directory(&mut result, module_root, upstream_root, &ignored);

    if json_output {
        match serde_json::to_string_pretty(&result) {
            Ok(json) => println!("{json}"),
            Err(error) => eprintln!("failed to serialize drift result: {error}"),
        }
    } else {
        print_drift_result(&result);
    }

    let has_drift = result.entries.iter().any(|entry| {
        matches!(
            entry.status,
            DriftStatus::FrontmatterOnly
                | DriftStatus::BodyOnly
                | DriftStatus::Both
                | DriftStatus::UpstreamOnly
        )
    });

    Ok(i32::from(has_drift))
}

// --- Comparison ---

fn compare_content_directory(
    result: &mut DriftResult,
    module_root: &Path,
    upstream_root: &Path,
    kind: ContentKind,
    ignored: &HashSet<&str>,
) {
    let module_directory = module_root.join(kind.as_str());
    let upstream_directory = upstream_root.join(kind.as_str());

    let module_files = collect_markdown_files(&module_directory);
    let upstream_files = collect_markdown_files(&upstream_directory);

    let all_names: BTreeSet<&String> = module_files.keys().chain(upstream_files.keys()).collect();

    for name in all_names {
        let entry = match (module_files.get(name), upstream_files.get(name)) {
            (Some(module_content), Some(upstream_content)) => compare_file_content(
                name,
                module_content,
                upstream_content,
                kind.as_str(),
                ignored,
            ),
            (Some(_), None) => DriftEntry {
                name: name.clone(),
                status: DriftStatus::LocalOnly,
                category: kind.as_str().to_string(),
                changed_keys: Vec::new(),
            },
            (None, Some(_)) => DriftEntry {
                name: name.clone(),
                status: DriftStatus::UpstreamOnly,
                category: kind.as_str().to_string(),
                changed_keys: Vec::new(),
            },
            (None, None) => continue,
        };

        result.entries.push(entry);
    }
}

fn compare_decisions_directory(
    result: &mut DriftResult,
    module_root: &Path,
    upstream_root: &Path,
    ignored: &HashSet<&str>,
) {
    let module_directory = module_root.join("docs/decisions");
    let upstream_directory = upstream_root.join("docs/decisions");

    if !module_directory.is_dir() && !upstream_directory.is_dir() {
        return;
    }

    let module_files = collect_markdown_files(&module_directory);
    let upstream_files = collect_markdown_files(&upstream_directory);

    let all_names: BTreeSet<&String> = module_files.keys().chain(upstream_files.keys()).collect();

    for name in all_names {
        let entry = match (module_files.get(name), upstream_files.get(name)) {
            (Some(module_content), Some(upstream_content)) => {
                compare_file_content(name, module_content, upstream_content, "decisions", ignored)
            }
            (Some(_), None) => DriftEntry {
                name: name.clone(),
                status: DriftStatus::LocalOnly,
                category: "decisions".to_string(),
                changed_keys: Vec::new(),
            },
            (None, Some(_)) => DriftEntry {
                name: name.clone(),
                status: DriftStatus::UpstreamOnly,
                category: "decisions".to_string(),
                changed_keys: Vec::new(),
            },
            (None, None) => continue,
        };

        result.entries.push(entry);
    }
}

fn compare_file_content(
    name: &str,
    module_content: &str,
    upstream_content: &str,
    category: &str,
    ignored: &HashSet<&str>,
) -> DriftEntry {
    let full_match = content_sha256(module_content) == content_sha256(upstream_content);
    if full_match {
        return DriftEntry {
            name: name.to_string(),
            status: DriftStatus::Identical,
            category: category.to_string(),
            changed_keys: Vec::new(),
        };
    }

    let (module_frontmatter, module_body) = split_parts(module_content);
    let (upstream_frontmatter, raw_upstream_body) = split_parts(upstream_content);

    // Assembly strips the leading `# Title` heading from deployed files.
    // Normalize both sides through the same transformation so drift
    // only fires on real content changes, not assembly artifacts.
    // This intentionally makes heading-only differences invisible —
    // including heading renames (e.g., `# OldName` → `# NewName`) —
    // because headings are derived from the `name` frontmatter field
    // during assembly, not authored body content.
    let normalized_upstream_body = strip_heading(raw_upstream_body);
    let normalized_module_body = strip_heading(module_body);

    let frontmatter_match =
        content_sha256(module_frontmatter) == content_sha256(upstream_frontmatter);
    let body_match =
        content_sha256(&normalized_module_body) == content_sha256(&normalized_upstream_body);

    let changed_keys = if frontmatter_match {
        Vec::new()
    } else {
        diff_frontmatter_keys(module_frontmatter, upstream_frontmatter)
    };

    let raw_status = match (frontmatter_match, body_match) {
        (true, true) => DriftStatus::Identical,
        (false, true) => DriftStatus::FrontmatterOnly,
        (true, false) => DriftStatus::BodyOnly,
        (false, false) => DriftStatus::Both,
    };

    let status = apply_ignore_filter(raw_status, &changed_keys, ignored);

    DriftEntry {
        name: name.to_string(),
        status,
        category: category.to_string(),
        changed_keys,
    }
}

fn apply_ignore_filter(
    raw_status: DriftStatus,
    changed_keys: &[String],
    ignored: &HashSet<&str>,
) -> DriftStatus {
    if ignored.is_empty() {
        return raw_status;
    }

    let body_ignored = ignored.contains(BODY_KEY);
    let all_keys_ignored = !changed_keys.is_empty()
        && changed_keys
            .iter()
            .all(|key| ignored.contains(key.as_str()));

    match raw_status {
        DriftStatus::FrontmatterOnly if all_keys_ignored => DriftStatus::Expected,
        DriftStatus::BodyOnly if body_ignored => DriftStatus::Expected,
        DriftStatus::Both if all_keys_ignored && body_ignored => DriftStatus::Expected,
        DriftStatus::Both if all_keys_ignored => DriftStatus::BodyOnly,
        DriftStatus::Both if body_ignored => DriftStatus::FrontmatterOnly,
        other => other,
    }
}

fn split_parts(content: &str) -> (&str, &str) {
    match split_frontmatter(content) {
        Some((frontmatter, body)) => (frontmatter, body),
        None => ("", content),
    }
}

fn diff_frontmatter_keys(module_yaml: &str, upstream_yaml: &str) -> Vec<String> {
    let module_map = parse_top_level_keys(module_yaml);
    let upstream_map = parse_top_level_keys(upstream_yaml);

    let all_keys: BTreeSet<&String> = module_map.keys().chain(upstream_map.keys()).collect();

    let mut changed = Vec::new();
    for key in all_keys {
        let module_value = module_map.get(key.as_str());
        let upstream_value = upstream_map.get(key.as_str());

        if module_value != upstream_value {
            changed.push(key.clone());
        }
    }
    changed
}

fn parse_top_level_keys(yaml_text: &str) -> BTreeMap<String, String> {
    let Ok(parsed): Result<serde_yaml::Value, _> = serde_yaml::from_str(yaml_text) else {
        return BTreeMap::new();
    };

    let Some(mapping) = parsed.as_mapping() else {
        return BTreeMap::new();
    };

    let mut result = BTreeMap::new();
    for (key, value) in mapping {
        if let Some(key_string) = key.as_str() {
            let serialized = serde_yaml::to_string(value).unwrap_or_default();
            result.insert(key_string.to_string(), serialized);
        }
    }
    result
}

// --- File Collection ---

fn collect_markdown_files(directory: &Path) -> BTreeMap<String, String> {
    let mut files = BTreeMap::new();

    if !directory.is_dir() {
        return files;
    }

    collect_markdown_recursive(directory, directory, &mut files);
    files
}

fn collect_markdown_recursive(
    base_directory: &Path,
    current_directory: &Path,
    files: &mut BTreeMap<String, String>,
) {
    let Ok(entries) = fs::read_dir(current_directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'))
        {
            continue;
        }

        if path.is_dir() {
            collect_markdown_recursive(base_directory, &path, files);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            let relative = path
                .strip_prefix(base_directory)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();

            if let Ok(content) = fs::read_to_string(&path) {
                files.insert(relative, content);
            }
        }
    }
}

// --- Output ---

fn print_drift_result(result: &DriftResult) {
    let categories: Vec<&str> = {
        let mut seen = Vec::new();
        for entry in &result.entries {
            if !seen.contains(&entry.category.as_str()) {
                seen.push(entry.category.as_str());
            }
        }
        seen
    };

    println!();
    for category in &categories {
        println!(" {}", Style::new().bold().apply_to(category));

        for entry in result
            .entries
            .iter()
            .filter(|entry| entry.category == *category)
        {
            print_drift_entry(entry);
        }
    }

    let red = Style::new().red();
    for error in &result.errors {
        println!("   {} {}", red.apply_to("✗"), red.apply_to(error));
    }

    print_drift_summary(result);
}

fn print_drift_entry(entry: &DriftEntry) {
    let green = Style::new().green();
    let dim = Style::new().dim();
    let cyan = Style::new().cyan();

    match entry.status {
        DriftStatus::Identical => {
            println!("   {} {}", green.apply_to("✓"), dim.apply_to(&entry.name));
        }
        DriftStatus::Expected => {
            println!("   {} {}", dim.apply_to("≈"), dim.apply_to(&entry.name),);
        }
        DriftStatus::FrontmatterOnly | DriftStatus::BodyOnly | DriftStatus::Both => {
            print_drift_card(entry);
        }
        DriftStatus::LocalOnly => {
            println!(
                "   {} {} {} {}",
                cyan.apply_to("●"),
                &entry.name,
                dim.apply_to("—"),
                cyan.apply_to("local only"),
            );
        }
        DriftStatus::UpstreamOnly => {
            println!(
                "   {} {} {} {}",
                dim.apply_to("○"),
                dim.apply_to(&entry.name),
                dim.apply_to("—"),
                dim.apply_to("upstream only"),
            );
        }
    }
}

fn print_drift_card(entry: &DriftEntry) {
    let green = Style::new().green();
    let yellow = Style::new().yellow();
    let dim = Style::new().dim();

    let frontmatter_drifted = matches!(
        entry.status,
        DriftStatus::FrontmatterOnly | DriftStatus::Both
    );
    let body_drifted = matches!(entry.status, DriftStatus::BodyOnly | DriftStatus::Both);

    println!("   {} {}", dim.apply_to("┌"), &entry.name);

    if frontmatter_drifted {
        let keys_display = if entry.changed_keys.is_empty() {
            "drifted".to_string()
        } else {
            entry.changed_keys.join(", ")
        };
        println!(
            "   {}  frontmatter  {} {}",
            dim.apply_to("│"),
            yellow.apply_to("⚡"),
            yellow.apply_to(keys_display),
        );
    } else {
        println!(
            "   {}  frontmatter  {}",
            dim.apply_to("│"),
            green.apply_to("✓"),
        );
    }

    if body_drifted {
        println!(
            "   {}  body         {} {}",
            dim.apply_to("│"),
            yellow.apply_to("⚡"),
            yellow.apply_to("drifted"),
        );
    } else {
        println!(
            "   {}  body         {}",
            dim.apply_to("│"),
            green.apply_to("✓"),
        );
    }

    println!("   {}", dim.apply_to("└"));
}

fn print_drift_summary(result: &DriftResult) {
    let green = Style::new().green();
    let yellow = Style::new().yellow();
    let cyan = Style::new().cyan();
    let dim = Style::new().dim();

    let mut identical_count = 0;
    let mut drifted_count = 0;
    let mut expected_count = 0;
    let mut local_count = 0;
    let mut upstream_count = 0;

    for entry in &result.entries {
        match entry.status {
            DriftStatus::Identical => identical_count += 1,
            DriftStatus::Expected => expected_count += 1,
            DriftStatus::LocalOnly => local_count += 1,
            DriftStatus::UpstreamOnly => upstream_count += 1,
            _ => drifted_count += 1,
        }
    }

    println!();
    let mut parts: Vec<String> = Vec::new();
    if identical_count > 0 {
        parts.push(format!(
            "{} {} identical",
            green.apply_to("✓"),
            identical_count
        ));
    }
    if drifted_count > 0 {
        parts.push(format!(
            "{} {} drifted",
            yellow.apply_to("⚡"),
            drifted_count
        ));
    }
    if expected_count > 0 {
        parts.push(format!("{} {} expected", dim.apply_to("≈"), expected_count));
    }
    if local_count > 0 {
        parts.push(format!("{} {} local", cyan.apply_to("●"), local_count));
    }
    if upstream_count > 0 {
        parts.push(format!("{} {} upstream", dim.apply_to("○"), upstream_count));
    }
    if !parts.is_empty() {
        println!(" {}", parts.join("  "));
    }
    println!();
}

#[cfg(test)]
mod tests;
