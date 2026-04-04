use super::*;
use tempfile::TempDir;

#[test]
fn create_tarball_produces_archive() {
    let temp_directory = TempDir::new().unwrap();
    let source_directory = temp_directory.path().join("source");
    std::fs::create_dir_all(&source_directory).unwrap();
    std::fs::write(source_directory.join("file.md"), "content").unwrap();

    let tarball_path = temp_directory.path().join("output.tar.gz");
    create_tarball(&source_directory, &tarball_path, "test").unwrap();

    assert!(tarball_path.exists());
    assert!(tarball_path.metadata().unwrap().len() > 0);
}

#[test]
fn create_tarball_creates_parent_directories() {
    let temp_directory = TempDir::new().unwrap();
    let source_directory = temp_directory.path().join("source");
    std::fs::create_dir_all(&source_directory).unwrap();
    std::fs::write(source_directory.join("file.md"), "content").unwrap();

    let tarball_path = temp_directory.path().join("nested/dir/output.tar.gz");
    create_tarball(&source_directory, &tarball_path, "test").unwrap();

    assert!(tarball_path.exists());
}

#[test]
fn create_tarball_errors_on_missing_source() {
    let temp_directory = TempDir::new().unwrap();
    let tarball_path = temp_directory.path().join("output.tar.gz");
    let result = create_tarball(
        &temp_directory.path().join("nonexistent"),
        &tarball_path,
        "test",
    );
    assert!(result.is_err());
}
