//! Filter a producer's full `Vec<SourceFile>` down to the subset requested
//! by a single source's `ArtifactList`. Records which requested names matched
//! and errors if any requested artifact failed to resolve.

use commands::error::{Error, ErrorKind};
use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use crate::cli::assemble::sources::SourceFile;
use crate::cli::dotforge::parse::ArtifactList;

pub fn filter_to_requested(
    all_files: Vec<SourceFile>,
    list: &ArtifactList,
    source_label: &str,
    source_path: &Path,
) -> Result<Vec<SourceFile>, Error> {
    let wanted_skills: HashSet<&str> = list.skills.iter().map(String::as_str).collect();
    let wanted_agents: HashSet<&str> = list.agents.iter().map(String::as_str).collect();
    let wanted_rules: HashSet<&str> = list.rules.iter().map(String::as_str).collect();

    let mut kept: Vec<SourceFile> = Vec::new();
    let mut seen_skills: BTreeSet<String> = BTreeSet::new();
    let mut seen_agents: BTreeSet<String> = BTreeSet::new();
    let mut seen_rules: BTreeSet<String> = BTreeSet::new();

    for file in all_files {
        if let Some(name) = skill_name(&file.relative_path)
            && wanted_skills.contains(name.as_str())
        {
            seen_skills.insert(name);
            kept.push(file);
        } else if let Some(name) = flat_name(&file.relative_path, "agents/")
            && wanted_agents.contains(name.as_str())
        {
            seen_agents.insert(name);
            kept.push(file);
        } else if let Some(name) = flat_name(&file.relative_path, "rules/")
            && wanted_rules.contains(name.as_str())
        {
            seen_rules.insert(name);
            kept.push(file);
        }
    }

    require_matched(
        "skill",
        &list.skills,
        &seen_skills,
        source_label,
        source_path,
    )?;
    require_matched(
        "agent",
        &list.agents,
        &seen_agents,
        source_label,
        source_path,
    )?;
    require_matched("rule", &list.rules, &seen_rules, source_label, source_path)?;

    Ok(kept)
}

fn skill_name(relative_path: &str) -> Option<String> {
    let stripped = relative_path.strip_prefix("skills/")?;
    let first = stripped.split('/').next()?;
    if first.is_empty() {
        None
    } else {
        Some(first.to_string())
    }
}

fn flat_name(relative_path: &str, prefix: &str) -> Option<String> {
    let stripped = relative_path.strip_prefix(prefix)?;
    if stripped.contains('/') {
        return None;
    }
    let name = stripped.strip_suffix(".md")?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn require_matched(
    kind: &str,
    requested: &[String],
    matched: &BTreeSet<String>,
    source_label: &str,
    source_path: &Path,
) -> Result<(), Error> {
    for name in requested {
        if !matched.contains(name) {
            return Err(Error::new(
                ErrorKind::Config,
                format!(
                    ".forge: {kind} '{name}' requested from source '{source_label}' not found at {}",
                    source_path.display()
                ),
            ));
        }
    }
    Ok(())
}
