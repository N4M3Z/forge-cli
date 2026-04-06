use super::*;
use commands::manifest::ManifestEntry;
use tempfile::TempDir;

fn template_content(filename: &str) -> String {
    let data = InitTemplates::get(filename).expect("template exists");
    std::str::from_utf8(data.data.as_ref())
        .expect("valid utf8")
        .to_string()
}

#[test]
fn warns_when_no_manifest() {
    let temp_directory = TempDir::new().unwrap();
    let mut result = ActionResult::new();

    check_template_drift(temp_directory.path(), &mut result);

    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("missing"));
}

#[test]
fn detects_matching_file() {
    let temp_directory = TempDir::new().unwrap();
    let content = template_content("LICENSE");
    let fingerprint = manifest::content_sha256(&content);

    std::fs::write(temp_directory.path().join("LICENSE"), &content).unwrap();

    let mut entries = std::collections::HashMap::new();
    entries.insert(
        "LICENSE".to_string(),
        ManifestEntry {
            fingerprint,
            provenance: None,
        },
    );
    let manifest_yaml = manifest::write(&entries).unwrap();
    std::fs::write(temp_directory.path().join(".manifest"), manifest_yaml).unwrap();

    let mut result = ActionResult::new();
    check_template_drift(temp_directory.path(), &mut result);

    assert!(result.warnings.is_empty());
}

#[test]
fn detects_drifted_file() {
    let temp_directory = TempDir::new().unwrap();
    let fingerprint = manifest::content_sha256(&template_content("LICENSE"));

    std::fs::write(temp_directory.path().join("LICENSE"), "modified content\n").unwrap();

    let mut entries = std::collections::HashMap::new();
    entries.insert(
        "LICENSE".to_string(),
        ManifestEntry {
            fingerprint,
            provenance: None,
        },
    );
    let manifest_yaml = manifest::write(&entries).unwrap();
    std::fs::write(temp_directory.path().join(".manifest"), manifest_yaml).unwrap();

    let mut result = ActionResult::new();
    check_template_drift(temp_directory.path(), &mut result);

    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("drifted"));
}

#[test]
fn detects_missing_tracked_file() {
    let temp_directory = TempDir::new().unwrap();

    let mut entries = std::collections::HashMap::new();
    entries.insert(
        "LICENSE".to_string(),
        ManifestEntry {
            fingerprint: "abc123".to_string(),
            provenance: None,
        },
    );
    let manifest_yaml = manifest::write(&entries).unwrap();
    std::fs::write(temp_directory.path().join(".manifest"), manifest_yaml).unwrap();

    let mut result = ActionResult::new();
    check_template_drift(temp_directory.path(), &mut result);

    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("missing"));
}
