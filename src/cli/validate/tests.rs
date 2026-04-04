use super::*;
use tempfile::TempDir;

#[test]
fn check_module_structure_reports_missing_required_files() {
    let temp_directory = TempDir::new().unwrap();
    let mut result = ActionResult::new();

    check_module_structure(temp_directory.path(), &mut result);

    assert_eq!(result.errors.len(), REQUIRED_FILES.len());
}

#[test]
fn check_module_structure_passes_with_all_required_files() {
    let temp_directory = TempDir::new().unwrap();

    for filename in REQUIRED_FILES {
        std::fs::write(temp_directory.path().join(filename), "content").unwrap();
    }

    let mut result = ActionResult::new();
    check_module_structure(temp_directory.path(), &mut result);

    assert!(result.errors.is_empty());
}

#[test]
fn check_module_yaml_validates_against_embedded_schema() {
    let temp_directory = TempDir::new().unwrap();
    let module_yaml = "name: test-module\nversion: 0.1.0\ndescription: test\nevents: []\n";
    std::fs::write(temp_directory.path().join("module.yaml"), module_yaml).unwrap();

    let mut result = ActionResult::new();
    check_module_yaml(temp_directory.path(), &mut result);

    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors
    );
}

#[test]
fn check_module_yaml_skips_when_no_module_yaml() {
    let temp_directory = TempDir::new().unwrap();
    let mut result = ActionResult::new();

    check_module_yaml(temp_directory.path(), &mut result);

    assert!(result.errors.is_empty());
}
