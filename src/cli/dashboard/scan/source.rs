//! Source and deployed artifact content reading, frontmatter parsing, sidecar
//! resolution, and manifest/key parsing.

use super::history::extract_frontmatter_field;
use commands::manifest::{self, ManifestEntry};
use commands::view::Companion;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Reads companion `.md` files in a source skill directory (everything
/// except `SKILL.md`), to fold under the parent skill.
pub(super) fn read_source_companions(skill_dir: &Path, skill_name: &str) -> Vec<Companion> {
    let Ok(entries) = fs::read_dir(skill_dir) else {
        return Vec::new();
    };
    let mut companions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
            continue;
        }
        let Some(stem) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
        else {
            continue;
        };
        if stem == "SKILL" {
            continue;
        }
        let raw_source = fs::read_to_string(&path).unwrap_or_default();
        companions.push(Companion {
            description: extract_frontmatter_field(&raw_source, "description"),
            content_body: strip_frontmatter(&raw_source),
            relative_path: format!("skills/{skill_name}/{stem}.md"),
            name: stem,
            raw_source,
        });
    }
    companions.sort_by(|a, b| a.name.cmp(&b.name));
    companions
}

/// Resolves a source artifact's provenance sidecar. The canonical name is
/// file-keyed (`SKILL.yaml` for `SKILL.md`, `<Companion>.yaml` for a companion)
/// so multiple files sharing one `.provenance` directory stay distinct. Older
/// copied modules used non-canonical names (`<file>.md.yaml`, or `<SkillName>.yaml`
/// keyed on the directory) which are tolerated as fallbacks for display.
pub(super) fn resolve_sidecar(parent_dir: &Path, source_path: &Path) -> Option<PathBuf> {
    let provenance = parent_dir.join(".provenance");
    let file_name = source_path.file_name()?.to_string_lossy().to_string();
    let file_stem = source_path.file_stem()?.to_string_lossy().to_string();
    let mut candidates: Vec<PathBuf> = Vec::new();
    candidates.push(provenance.join(format!("{file_stem}.yaml")));
    candidates.push(provenance.join(format!("{file_name}.yaml")));
    if file_name == "SKILL.md"
        && let Some(dir_name) = source_path
            .parent()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().to_string())
    {
        candidates.push(provenance.join(format!("{dir_name}.yaml")));
    }
    candidates.into_iter().find(|candidate| candidate.is_file())
}

pub(super) struct SourceContent {
    pub(super) description: String,
    pub(super) body: String,
    pub(super) raw: String,
    pub(super) metadata: Vec<(String, String)>,
}

pub(super) fn read_source_content(
    source_uri: &str,
    source_path: Option<&str>,
    local_repos: &HashMap<String, PathBuf>,
) -> SourceContent {
    let empty = SourceContent {
        description: String::new(),
        body: String::new(),
        raw: String::new(),
        metadata: Vec::new(),
    };
    let normalized = source_uri.trim_end_matches(".git");
    let Some(repo_path) = local_repos.get(normalized) else {
        return empty;
    };
    let Some(file_rel) = source_path else {
        return empty;
    };
    let file_path = repo_path.join(file_rel);
    let Ok(content) = fs::read_to_string(&file_path) else {
        return empty;
    };
    let description = extract_frontmatter_field(&content, "description");
    let metadata = parse_frontmatter(&content);
    let body = strip_frontmatter(&content);
    SourceContent {
        description,
        body,
        raw: content,
        metadata,
    }
}

/// Parses flat frontmatter fields, preserving their source order for display.
pub(super) fn parse_frontmatter(content: &str) -> Vec<(String, String)> {
    let mut fields = Vec::new();
    let Some(rest) = content.strip_prefix("---") else {
        return fields;
    };
    let Some(end) = rest.find("\n---") else {
        return fields;
    };
    let frontmatter = &rest[..end];
    for line in frontmatter.lines() {
        let trimmed = line.trim();
        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim().to_string();
            let value = value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();
            if !key.is_empty() && !value.is_empty() {
                fields.push((key, value));
            }
        }
    }
    fields
}

pub(super) struct ArtifactContent {
    pub(super) description: String,
    pub(super) body: String,
}

pub(super) fn read_artifact_content(provider_path: &Path, relative_key: &str) -> ArtifactContent {
    let file_path = provider_path.join(relative_key);
    let Ok(content) = fs::read_to_string(&file_path) else {
        return ArtifactContent {
            description: String::new(),
            body: String::new(),
        };
    };
    let description = extract_frontmatter_field(&content, "description");
    let body = strip_frontmatter(&content);
    ArtifactContent { description, body }
}

pub(super) fn strip_frontmatter(content: &str) -> String {
    let Some(rest) = content.strip_prefix("---") else {
        return content.to_string();
    };
    let Some(end) = rest.find("\n---") else {
        return content.to_string();
    };
    rest[end + 4..].trim_start().to_string()
}

pub(super) fn resolve_source(
    provider_path: &Path,
    relative_key: &str,
    entry: &ManifestEntry,
) -> String {
    if let Some(ref provenance_rel) = entry.provenance {
        let sidecar_path = provider_path.join(provenance_rel);
        if let Ok(content) = fs::read_to_string(&sidecar_path)
            && let Some(source_uri) = super::provenance::extract_source_uri(&content)
        {
            return source_uri;
        }
    }
    let target_label = provider_path
        .parent()
        .and_then(|parent| parent.file_name())
        .map_or_else(
            || "unknown".to_string(),
            |name| name.to_string_lossy().to_string(),
        );
    format!(
        "{target_label}/{}",
        relative_key.split('/').next().unwrap_or("unknown")
    )
}

pub(super) fn resolve_source_name(provider_path: &Path, entry: &ManifestEntry) -> Option<String> {
    let provenance_rel = entry.provenance.as_ref()?;
    let sidecar_path = provider_path.join(provenance_rel);
    let content = fs::read_to_string(&sidecar_path).ok()?;
    extract_dependency_uri(&content)
}

pub(super) fn resolve_source_path(provider_path: &Path, entry: &ManifestEntry) -> Option<String> {
    let provenance_rel = entry.provenance.as_ref()?;
    let sidecar_path = provider_path.join(provenance_rel);
    let content = fs::read_to_string(&sidecar_path).ok()?;
    for line in content.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        if let Some(uri) = trimmed.strip_prefix("uri:") {
            return Some(uri.trim().to_string());
        }
    }
    None
}

pub(super) fn extract_dependency_uri(sidecar_content: &str) -> Option<String> {
    for line in sidecar_content.lines() {
        let trimmed = line.trim().trim_start_matches("- ");
        if let Some(uri) = trimmed.strip_prefix("uri:") {
            let path = uri.trim();
            let segments: Vec<&str> = path.split('/').collect();
            let filename = segments.last().unwrap_or(&path);
            let stem = filename.trim_end_matches(".md").trim_end_matches(".toml");
            if stem == "SKILL" && segments.len() >= 3 {
                return Some(segments[segments.len() - 2].to_string());
            }
            return Some(stem.to_string());
        }
    }
    None
}

pub(super) fn load_manifest(target_dir: &Path) -> HashMap<String, ManifestEntry> {
    let manifest_path = target_dir.join(".manifest");
    let Ok(content) = fs::read_to_string(&manifest_path) else {
        return HashMap::new();
    };
    manifest::read(&content).unwrap_or_default()
}

pub(super) fn parse_artifact_key(key: &str) -> Option<(&str, String)> {
    let parts: Vec<&str> = key.splitn(3, '/').collect();
    if parts.len() < 2 {
        return None;
    }
    let kind = match parts[0] {
        "skills" | "agents" | "rules" => parts[0],
        _ => return None,
    };
    let name = parts[1].trim_end_matches(".md").trim_end_matches(".toml");
    Some((kind, name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    #[test]
    fn resolve_sidecar_prefers_canonical_skill_yaml() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skills/Foo");
        write(&skill_dir.join(".provenance/SKILL.yaml"), "a: 1\n");
        write(&skill_dir.join(".provenance/Foo.yaml"), "a: 2\n");
        let resolved = resolve_sidecar(&skill_dir, Path::new("skills/Foo/SKILL.md")).unwrap();
        assert!(resolved.ends_with("SKILL.yaml"));
    }

    #[test]
    fn resolve_sidecar_falls_back_to_legacy_dir_name() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skills/LearnFrom");
        write(&skill_dir.join(".provenance/LearnFrom.yaml"), "a: 1\n");
        let resolved = resolve_sidecar(&skill_dir, Path::new("skills/LearnFrom/SKILL.md")).unwrap();
        assert!(resolved.ends_with("LearnFrom.yaml"));
    }

    #[test]
    fn resolve_sidecar_flat_artifact_uses_stem() {
        let temp = TempDir::new().unwrap();
        let kind_dir = temp.path().join("rules");
        write(&kind_dir.join(".provenance/Bar.yaml"), "a: 1\n");
        let resolved = resolve_sidecar(&kind_dir, Path::new("rules/Bar.md")).unwrap();
        assert!(resolved.ends_with("Bar.yaml"));
    }

    #[test]
    fn resolve_sidecar_none_when_absent() {
        let temp = TempDir::new().unwrap();
        let skill_dir = temp.path().join("skills/Empty");
        std::fs::create_dir_all(skill_dir.join(".provenance")).unwrap();
        assert!(resolve_sidecar(&skill_dir, Path::new("skills/Empty/SKILL.md")).is_none());
    }
}
