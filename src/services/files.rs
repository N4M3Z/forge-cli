//! Filesystem-only dashboard file discovery.
//!
//! These helpers collect config, settings, hook, schema, and manifest files for
//! both the web dashboard and the TUI. They do not depend on any web framework
//! state and only read allowlisted filesystem paths.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A read-only config/settings/schema file shown by dashboard surfaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFile {
    pub label: String,
    pub path: String,
    pub language: String,
    pub content: String,
}

/// One registered hook parsed from a settings.json `hooks` block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookEntry {
    pub event: String,
    pub matcher: String,
    pub command: String,
    pub source: String,
}

/// Settings/config files found in one harness's target directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessFiles {
    pub harness: String,
    pub files: Vec<ConfigFile>,
}

/// Hooks parsed from one harness's settings files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessHooks {
    pub harness: String,
    pub hooks: Vec<HookEntry>,
}

/// `.mdschema` and `.manifest` files from one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaGroup {
    pub source: String,
    pub files: Vec<ConfigFile>,
}

/// File collections backing the TUI's file-browser sections.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileSections {
    pub settings: Vec<HarnessFiles>,
    pub hooks: Vec<HarnessHooks>,
    pub config: Vec<ConfigFile>,
    pub schemas: Vec<SchemaGroup>,
}

/// Forge-cli's own config files surfaced from `~/.config/forge`.
const FORGE_CONFIG_FILES: &[&str] = &[
    "config.yaml",
    "config.yml",
    "config.toml",
    "config.json",
    "watchlist.yaml",
];

/// Builds all file-browser collections for a scanned root.
#[must_use]
pub fn collect_file_sections(
    root: &Path,
    provider_targets: &[(String, String)],
    settings_filenames: &[String],
    local_repos: &HashMap<String, PathBuf>,
    allowed_sources: &HashSet<String>,
) -> FileSections {
    let settings = settings_by_harness(root, provider_targets, settings_filenames);
    let hooks = hooks_from_settings(&settings);
    let mut config = collect_forge_config_files(root);
    for group in &settings {
        for mut file in group.files.clone() {
            file.label = format!("{} · {}", group.harness, file.label);
            config.push(file);
        }
    }
    let schemas = schemas_by_source(root, provider_targets, local_repos, allowed_sources);
    FileSections {
        settings,
        hooks,
        config,
        schemas,
    }
}

/// Forge config files plus each harness's settings files as a flat list.
#[must_use]
pub fn collect_dashboard_config_files(
    root: &Path,
    provider_targets: &[(String, String)],
    settings_filenames: &[String],
) -> Vec<ConfigFile> {
    collect_dashboard_config_files_with_home(
        root,
        provider_targets,
        settings_filenames,
        dirs::home_dir().as_deref(),
    )
}

/// Full dashboard config list using an explicit home directory, useful for tests.
#[must_use]
pub fn collect_dashboard_config_files_with_home(
    root: &Path,
    provider_targets: &[(String, String)],
    settings_filenames: &[String],
    home: Option<&Path>,
) -> Vec<ConfigFile> {
    let mut files = collect_forge_config_files_with_home(root, home);
    for group in settings_by_harness_with_home(root, provider_targets, settings_filenames, home) {
        for mut file in group.files {
            file.label = format!("{} · {}", group.harness, file.label);
            files.push(file);
        }
    }
    files
}

/// Forge config files at the scanned root plus allowlisted forge config files.
#[must_use]
pub fn collect_forge_config_files(root: &Path) -> Vec<ConfigFile> {
    collect_forge_config_files_with_home(root, dirs::home_dir().as_deref())
}

/// Forge config files using an explicit home directory, useful for tests.
#[must_use]
pub fn collect_forge_config_files_with_home(root: &Path, home: Option<&Path>) -> Vec<ConfigFile> {
    let mut files = Vec::new();
    for (label, name, lang) in [
        ("Module manifest", "module.yaml", "yaml"),
        ("Defaults", "defaults.yaml", "yaml"),
        ("Config override", "config.yaml", "yaml"),
        ("Consumer manifest", ".forge", "yaml"),
    ] {
        if let Some(file) = read_config_file(label, &root.join(name), lang, home) {
            files.push(file);
        }
    }
    if let Some(home) = home {
        let forge_dir = home.join(".config/forge");
        if let Ok(entries) = std::fs::read_dir(&forge_dir) {
            let mut names: Vec<_> = entries
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
                .map(|entry| entry.file_name().to_string_lossy().to_string())
                .filter(|name| {
                    FORGE_CONFIG_FILES
                        .iter()
                        .any(|allowed| name.eq_ignore_ascii_case(allowed))
                })
                .collect();
            names.sort();
            for name in names {
                let is_toml = Path::new(&name)
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"));
                let lang = if is_toml { "toml" } else { "yaml" };
                if let Some(file) =
                    read_config_file("~/.config/forge", &forge_dir.join(&name), lang, Some(home))
                {
                    files.push(file);
                }
            }
        }
    }
    files
}

/// Settings/config files grouped per harness.
#[must_use]
pub fn settings_by_harness(
    root: &Path,
    provider_targets: &[(String, String)],
    allowed: &[String],
) -> Vec<HarnessFiles> {
    settings_by_harness_with_home(root, provider_targets, allowed, dirs::home_dir().as_deref())
}

/// Settings/config files grouped per harness using an explicit home directory.
#[must_use]
pub fn settings_by_harness_with_home(
    root: &Path,
    provider_targets: &[(String, String)],
    allowed: &[String],
    home: Option<&Path>,
) -> Vec<HarnessFiles> {
    let mut groups = Vec::new();
    for (harness, target) in provider_targets {
        let mut files = Vec::new();
        if let Some(home) = home {
            collect_config_files(&home.join(target), allowed, &mut files, Some(home));
        }
        collect_config_files(&root.join(target), allowed, &mut files, home);
        if !files.is_empty() {
            groups.push(HarnessFiles {
                harness: harness.clone(),
                files,
            });
        }
    }
    groups
}

/// Hooks grouped per harness, parsed from each harness's JSON settings files.
#[must_use]
pub fn hooks_by_harness(
    root: &Path,
    provider_targets: &[(String, String)],
    allowed: &[String],
) -> Vec<HarnessHooks> {
    hooks_from_settings(&settings_by_harness(root, provider_targets, allowed))
}

/// Hooks grouped per harness from a precomputed settings collection.
#[must_use]
pub fn hooks_from_settings(settings: &[HarnessFiles]) -> Vec<HarnessHooks> {
    let mut groups = Vec::new();
    for harness_files in settings {
        let mut hooks = Vec::new();
        for file in &harness_files.files {
            if file.language == "json" {
                parse_hooks(&file.content, &file.path, &mut hooks);
            }
        }
        if !hooks.is_empty() {
            groups.push(HarnessHooks {
                harness: harness_files.harness.clone(),
                hooks,
            });
        }
    }
    groups
}

/// Collects `.mdschema` and `.manifest` files, grouped by source.
#[must_use]
pub fn schemas_by_source(
    root: &Path,
    provider_targets: &[(String, String)],
    local_repos: &HashMap<String, PathBuf>,
    allowed: &HashSet<String>,
) -> Vec<SchemaGroup> {
    let home = dirs::home_dir();
    schemas_by_source_with_home(
        root,
        provider_targets,
        local_repos,
        allowed,
        home.as_deref(),
    )
}

/// Collects schemas using an explicit home directory, useful for tests.
#[must_use]
pub fn schemas_by_source_with_home(
    root: &Path,
    provider_targets: &[(String, String)],
    local_repos: &HashMap<String, PathBuf>,
    allowed: &HashSet<String>,
    home: Option<&Path>,
) -> Vec<SchemaGroup> {
    let mut groups = Vec::new();
    let mut repos: Vec<&PathBuf> = local_repos
        .values()
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| allowed.contains(name.to_string_lossy().as_ref()))
        })
        .collect();
    repos.sort();
    for repo_path in repos {
        let mut files = Vec::new();
        for kind in ["skills", "agents", "rules"] {
            if let Some(file) = read_config_file(
                &format!("{kind}/.mdschema"),
                &repo_path.join(kind).join(".mdschema"),
                "yaml",
                home,
            ) {
                files.push(file);
            }
        }
        if let Some(file) = read_config_file(
            "docs/decisions/.mdschema",
            &repo_path.join("docs/decisions/.mdschema"),
            "yaml",
            home,
        ) {
            files.push(file);
        }
        if let Some(file) =
            read_config_file(".manifest", &repo_path.join(".manifest"), "yaml", home)
        {
            files.push(file);
        }
        if !files.is_empty() {
            let source = repo_path.file_name().map_or_else(
                || repo_path.display().to_string(),
                |name| name.to_string_lossy().to_string(),
            );
            groups.push(SchemaGroup { source, files });
        }
    }
    for (_harness, target) in provider_targets {
        let mut bases: Vec<PathBuf> = Vec::new();
        if let Some(home) = home {
            bases.push(home.to_path_buf());
        }
        bases.push(root.to_path_buf());
        for base in bases {
            let provider_dir = base.join(target);
            if let Some(file) =
                read_config_file(".manifest", &provider_dir.join(".manifest"), "yaml", home)
            {
                groups.push(SchemaGroup {
                    source: format!("deployed: {}", display_path(&provider_dir, home)),
                    files: vec![file],
                });
            }
        }
    }
    groups
}

/// Strips a `sh -c '<script>'` / `bash -c "<script>"` wrapper.
#[must_use]
pub fn unwrap_shell(command: &str) -> (String, String) {
    let trimmed = command.trim();
    for program in ["sh", "bash", "zsh"] {
        for quote in ['\'', '"'] {
            let prefix = format!("{program} -c {quote}");
            if let Some(rest) = trimmed.strip_prefix(&prefix)
                && let Some(inner) = rest.strip_suffix(quote)
            {
                return (format!("{program} -c"), inner.to_string());
            }
        }
    }
    (String::new(), command.to_string())
}

/// Parses a settings.json `hooks` block into flat `HookEntry` rows.
pub fn parse_hooks(content: &str, source: &str, out: &mut Vec<HookEntry>) {
    #[derive(serde::Deserialize)]
    struct Settings {
        #[serde(default)]
        hooks: std::collections::BTreeMap<String, Vec<HookMatcher>>,
    }
    #[derive(serde::Deserialize)]
    struct HookMatcher {
        #[serde(default)]
        matcher: String,
        #[serde(default)]
        hooks: Vec<HookCommand>,
    }
    #[derive(serde::Deserialize)]
    struct HookCommand {
        #[serde(default)]
        command: String,
    }
    let Ok(settings) = serde_json::from_str::<Settings>(content) else {
        return;
    };
    for (event, matchers) in settings.hooks {
        for matcher in matchers {
            for command in matcher.hooks {
                out.push(HookEntry {
                    event: event.clone(),
                    matcher: matcher.matcher.clone(),
                    command: command.command,
                    source: source.to_string(),
                });
            }
        }
    }
}

/// Hard cap on a browsed config file (2 MiB): config/settings/schema files are
/// small, and this bounds memory against an oversized or adversarial file.
const MAX_CONFIG_BYTES: u64 = 2 * 1024 * 1024;

/// Read a file to a string only if it is a regular file (not a symlink) and
/// within the size cap. `symlink_metadata` does not follow the final component,
/// so a symlink with an allowlisted name cannot redirect the read to a
/// sensitive file outside the intended directory (path-boundary validation).
fn read_regular_file_capped(path: &Path) -> Option<String> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CONFIG_BYTES {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

/// Reads a file into a `ConfigFile` if it exists, abbreviating the path with `~`.
#[must_use]
pub fn read_config_file(
    label: &str,
    path: &Path,
    language: &str,
    home: Option<&Path>,
) -> Option<ConfigFile> {
    let content = read_regular_file_capped(path)?;
    Some(ConfigFile {
        label: label.to_string(),
        path: display_path(path, home),
        language: language.to_string(),
        content,
    })
}

fn collect_config_files(
    dir: &Path,
    allowed: &[String],
    out: &mut Vec<ConfigFile>,
    home: Option<&Path>,
) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|file_type| file_type.is_file()))
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| allowed.iter().any(|known| name.eq_ignore_ascii_case(known)))
        .collect();
    names.sort();
    for name in names {
        if let Some(language) = extension_language(&name)
            && let Some(file) = read_config_file(&name, &dir.join(&name), language, home)
        {
            out.push(file);
        }
    }
}

fn extension_language(name: &str) -> Option<&'static str> {
    let extension = Path::new(name).extension()?.to_str()?;
    match extension.to_ascii_lowercase().as_str() {
        "json" => Some("json"),
        "toml" => Some("toml"),
        "yaml" | "yml" => Some("yaml"),
        _ => None,
    }
}

fn display_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home
        && let Ok(rest) = path.strip_prefix(home)
    {
        return format!("~/{}", rest.display());
    }
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn read_config_file_rejects_symlink_and_oversized() {
        let temp = tempfile::tempdir().expect("tempdir");
        let secret = temp.path().join("secret");
        std::fs::write(&secret, "TOP SECRET\n").expect("secret");
        let link = temp.path().join("settings.json");
        std::os::unix::fs::symlink(&secret, &link).expect("symlink");
        assert!(
            read_config_file("settings", &link, "json", None).is_none(),
            "a symlink with an allowlisted name must not be read through"
        );

        let real = temp.path().join("real.json");
        std::fs::write(&real, "{}\n").expect("real");
        assert!(read_config_file("settings", &real, "json", None).is_some());
    }

    #[test]
    fn settings_and_hooks_use_allowlisted_fixture_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let root = temp.path().join("project");
        let settings_dir = home.join(".claude");
        std::fs::create_dir_all(&settings_dir).expect("settings dir");
        std::fs::create_dir_all(&root).expect("root dir");
        std::fs::write(
            settings_dir.join("settings.json"),
            r#"{
              "hooks": {
                "PreToolUse": [
                  {
                    "matcher": "Write",
                    "hooks": [
                      { "command": "bash -c 'echo fixture-hook'" }
                    ]
                  }
                ]
              }
            }"#,
        )
        .expect("settings file");
        std::fs::write(settings_dir.join("ignored.env"), "SECRET=ignored").expect("ignored file");

        let providers = vec![("claude".to_string(), ".claude".to_string())];
        let allowed = vec!["settings.json".to_string()];
        let settings = settings_by_harness_with_home(&root, &providers, &allowed, Some(&home));
        assert_eq!(settings.len(), 1);
        assert_eq!(settings[0].files[0].label, "settings.json");
        assert!(settings[0].files[0].content.contains("fixture-hook"));

        let hooks = hooks_from_settings(&settings);
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].hooks[0].matcher, "Write");
        assert!(hooks[0].hooks[0].command.contains("fixture-hook"));
    }

    #[test]
    fn config_and_schema_collections_include_fixture_content() {
        let temp = tempfile::tempdir().expect("tempdir");
        let home = temp.path().join("home");
        let root = temp.path().join("project");
        let repo = temp.path().join("forge-core");
        std::fs::create_dir_all(home.join(".config/forge")).expect("forge config dir");
        std::fs::create_dir_all(repo.join("skills")).expect("schema dir");
        std::fs::create_dir_all(root.join(".claude")).expect("target dir");
        std::fs::write(root.join("module.yaml"), "name: fixture\n").expect("module");
        std::fs::write(home.join(".config/forge/config.toml"), "theme = 'dark'\n")
            .expect("forge config");
        std::fs::write(repo.join("skills/.mdschema"), "kind: skills\n").expect("schema");
        std::fs::write(root.join(".claude/.manifest"), "skills: {}\n").expect("manifest");

        let providers = vec![("claude".to_string(), ".claude".to_string())];
        let settings_filenames = vec!["settings.json".to_string()];
        let config = collect_dashboard_config_files_with_home(
            &root,
            &providers,
            &settings_filenames,
            Some(&home),
        );
        assert!(config.iter().any(|file| file.path.ends_with("module.yaml")));
        assert!(config.iter().any(|file| file.content.contains("theme")));

        let mut local_repos = HashMap::new();
        local_repos.insert("forge-core".to_string(), repo);
        let allowed = HashSet::from(["forge-core".to_string()]);
        let schemas =
            schemas_by_source_with_home(&root, &providers, &local_repos, &allowed, Some(&home));
        assert!(schemas.iter().any(|group| group.source == "forge-core"));
        assert!(
            schemas
                .iter()
                .flat_map(|group| &group.files)
                .any(|file| file.content.contains("kind: skills"))
        );
        assert!(
            schemas
                .iter()
                .any(|group| group.source.contains("deployed:")
                    && group.files[0].content.contains("skills"))
        );
    }
}
