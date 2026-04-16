use super::*;
use tempfile::TempDir;

#[test]
fn create_tarball_produces_archive() {
    let temp_directory = TempDir::new().unwrap();
    let wrapper = temp_directory.path().join("my-module");
    std::fs::create_dir_all(&wrapper).unwrap();
    std::fs::write(wrapper.join("file.md"), "content").unwrap();

    let tarball_path = temp_directory.path().join("output.tar.gz");
    create_tarball(temp_directory.path(), &tarball_path, "my-module", "test").unwrap();

    assert!(tarball_path.exists());
    assert!(tarball_path.metadata().unwrap().len() > 0);
}

#[test]
fn create_tarball_errors_on_missing_source() {
    let temp_directory = TempDir::new().unwrap();
    let tarball_path = temp_directory.path().join("output.tar.gz");
    let result = create_tarball(temp_directory.path(), &tarball_path, "nonexistent", "test");
    assert!(result.is_err());
}

#[test]
fn makefile_template_substitutes_provider() {
    let content = MAKEFILE_TEMPLATE.replace("${PROVIDER}", "claude");
    assert!(content.contains(".claude"));
    assert!(!content.contains("${PROVIDER}"));
}

#[test]
fn makefile_template_uses_tabs_for_recipes() {
    let content = MAKEFILE_TEMPLATE.replace("${PROVIDER}", "claude");
    let recipe_lines: Vec<&str> = content
        .lines()
        .filter(|line| line.starts_with('\t'))
        .collect();
    assert!(!recipe_lines.is_empty(), "Makefile recipes must use tabs");
}
