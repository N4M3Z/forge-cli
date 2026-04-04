use super::*;
use tempfile::TempDir;

#[test]
fn execute_errors_on_missing_module() {
    let result = execute("/nonexistent/module", None, false, false, false);
    assert!(result.is_err());
}

#[test]
fn execute_succeeds_on_empty_module() {
    let temp_directory = TempDir::new().unwrap();
    std::fs::write(
        temp_directory.path().join("defaults.yaml"),
        "providers:\n    claude:\n        target: .claude\n",
    )
    .unwrap();
    std::fs::write(
        temp_directory.path().join("module.yaml"),
        "name: test\nversion: 0.1.0\ndescription: test\nevents: []\n",
    )
    .unwrap();

    let target = TempDir::new().unwrap();
    let result = execute(
        &temp_directory.path().to_string_lossy(),
        Some(&target.path().to_string_lossy()),
        false,
        false,
        false,
    );
    assert!(result.is_ok());
}
