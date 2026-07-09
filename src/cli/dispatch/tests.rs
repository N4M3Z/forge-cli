use super::*;

#[test]
fn forge_root_uses_nearest_ancestor_with_module_yaml() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("root");
    let child = root.join("a/b");
    std::fs::create_dir_all(&child).expect("create child");
    std::fs::write(root.join("module.yaml"), "name: test\n").expect("module");

    assert_eq!(forge_root_from(&child).expect("root"), root);
}

#[test]
fn resolve_external_prefers_root_commands_then_extensions() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("root");
    let extension = dir.path().join("extension");
    std::fs::create_dir_all(root.join("commands")).expect("root commands");
    std::fs::create_dir_all(&extension).expect("extension");
    let root_command = root.join("commands/forge-hello");
    let extension_command = extension.join("forge-hello");
    std::fs::write(&root_command, "#!/usr/bin/env bash\n").expect("root script");
    std::fs::write(&extension_command, "#!/usr/bin/env bash\n").expect("extension script");

    assert_eq!(
        resolve_external("forge-hello", &root, &[extension]),
        Some(root_command)
    );
}
