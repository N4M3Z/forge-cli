# Flatten Skill Subdirectories Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix `walk_skill_dir()` to collect files from `user/` subdirectories and flatten them into the skill root during assembly, with `user/` files overriding root files on name collision.

**Architecture:** Change `walk_skill_dir()` in `sources.rs` to walk subdirectories instead of skipping them. Root files are collected first, then `user/` files overwrite any collisions. Assembly output logs flattened files. One function change, one test fixture, one integration test.

**Tech Stack:** Rust, `std::fs`, existing `SourceFile` struct

---

### Task 1: Add test fixture — skill with user/ companion

**Files:**
- Create: `tests/fixtures/input/skill-user-companion.md`
- Create: `tests/fixtures/input/skill-user-override.md`

- [ ] **Step 1: Create the user-only companion fixture**

```markdown
This is a test companion file for validating that user/ subdirectory files are flattened into the skill root during assembly.
```

Save to `tests/fixtures/input/skill-user-companion.md`.

- [ ] **Step 2: Create the user override fixture**

```markdown
This is a test override file for validating that user/ files take precedence over root files with the same name during assembly.
```

Save to `tests/fixtures/input/skill-user-override.md`.

- [ ] **Step 3: Commit fixtures**

```sh
git add tests/fixtures/input/skill-user-companion.md tests/fixtures/input/skill-user-override.md
git commit -m "test: add fixtures for skill user/ subdirectory flattening"
```

---

### Task 2: Write failing integration test

**Files:**
- Modify: `src/cli/assemble/sources.rs` (add test at bottom of file)

- [ ] **Step 1: Write the test for user/ companion flattening**

Add to the `#[cfg(test)] mod tests` block at the bottom of `sources.rs`:

```rust
#[test]
fn walk_skill_dir_flattens_user_subdirectory() {
    use std::fs;
    use tempfile::TempDir;

    let temp = TempDir::new().unwrap();
    let skill_dir = temp.path().join("skills").join("TestSkill");
    let user_dir = skill_dir.join("user");
    fs::create_dir_all(&user_dir).unwrap();

    // Root files
    fs::write(skill_dir.join("SKILL.md"), "---\nname: TestSkill\n---\n# TestSkill").unwrap();
    fs::write(skill_dir.join("Reference.md"), "Root reference content").unwrap();

    // user/ files: one addition, one override
    fs::write(user_dir.join("Extra.md"), "User-only companion").unwrap();
    fs::write(user_dir.join("Reference.md"), "User override content").unwrap();

    let mut sources = Vec::new();
    walk_skill_dir(
        &skill_dir,
        commands::provider::ContentKind::Skills,
        temp.path(),
        &mut sources,
    )
    .unwrap();

    // Should have 3 files: SKILL.md, Reference.md (user/ override), Extra.md (user/ addition)
    assert_eq!(sources.len(), 3, "expected 3 sources, got {}", sources.len());

    let skill = sources.iter().find(|s| s.relative_path.contains("SKILL.md")).unwrap();
    assert!(!skill.passthrough, "SKILL.md should not be passthrough");

    let reference = sources.iter().find(|s| s.relative_path.contains("Reference.md")).unwrap();
    assert!(reference.passthrough, "Reference.md should be passthrough");
    assert_eq!(reference.content, "User override content", "user/ should override root");

    let extra = sources.iter().find(|s| s.relative_path.contains("Extra.md")).unwrap();
    assert!(extra.passthrough, "Extra.md should be passthrough");
    assert_eq!(extra.content, "User-only companion");

    // All paths should be flattened (no "user/" in relative path)
    for source in &sources {
        assert!(
            !source.relative_path.contains("user/"),
            "relative path should be flattened: {}",
            source.relative_path
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```sh
TMPDIR=/private/tmp/claude-501 cargo test --manifest-path /Users/N4M3Z/Data/Modules/forge-cli/Cargo.toml -- walk_skill_dir_flattens -v
```

Expected: FAIL — `walk_skill_dir` currently skips directories, so `user/` files are not collected.

- [ ] **Step 3: Commit failing test**

```sh
git add src/cli/assemble/sources.rs
git commit -m "test: add failing test for skill user/ subdirectory flattening"
```

---

### Task 3: Implement flattening in walk_skill_dir

**Files:**
- Modify: `src/cli/assemble/sources.rs:229-275`

- [ ] **Step 1: Rewrite walk_skill_dir to collect from subdirectories**

Replace the current `walk_skill_dir` function (lines 229-275) with:

```rust
fn walk_skill_dir(
    dir: &Path,
    kind: commands::provider::ContentKind,
    module_root: &Path,
    sources: &mut Vec<SourceFile>,
) -> Result<(), Error> {
    let skill_name = dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    // Collect root files first (keyed by filename for override detection)
    let mut file_map: std::collections::HashMap<String, SourceFile> =
        std::collections::HashMap::new();

    collect_skill_files(dir, &skill_name, kind, module_root, &mut file_map, false)?;

    // Then collect user/ files (overrides root on collision)
    let user_dir = dir.join("user");
    if user_dir.is_dir() {
        collect_skill_files(&user_dir, &skill_name, kind, module_root, &mut file_map, true)?;
    }

    sources.extend(file_map.into_values());
    Ok(())
}

fn collect_skill_files(
    dir: &Path,
    skill_name: &str,
    kind: commands::provider::ContentKind,
    module_root: &Path,
    file_map: &mut std::collections::HashMap<String, SourceFile>,
    is_overlay: bool,
) -> Result<(), Error> {
    let entries = fs::read_dir(dir)
        .map_err(|e| Error::new(ErrorKind::Io, format!("cannot read {}: {e}", dir.display())))?;

    for entry in entries {
        let entry =
            entry.map_err(|e| Error::new(ErrorKind::Io, format!("directory entry error: {e}")))?;

        let path = entry.path();

        if path.is_dir() || path.extension().unwrap_or_default() != "md" {
            continue;
        }

        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let is_skill_file = filename == "SKILL.md";

        // Flatten: use skills/<SkillName>/<filename> regardless of source subdirectory
        let flattened_relative = format!("skills/{skill_name}/{filename}");

        let content = read_file(&path)?;

        if is_overlay && file_map.contains_key(&filename) {
            eprintln!("  override  skills/{skill_name}/user/{filename} → {filename}");
        } else if is_overlay {
            eprintln!("  flatten   skills/{skill_name}/user/{filename} → {filename}");
        }

        file_map.insert(
            filename,
            SourceFile {
                relative_path: flattened_relative,
                full_path: path.to_string_lossy().to_string(),
                content,
                kind,
                passthrough: !is_skill_file,
                qualifier: None,
            },
        );
    }

    Ok(())
}
```

- [ ] **Step 2: Run the test**

```sh
TMPDIR=/private/tmp/claude-501 cargo test --manifest-path /Users/N4M3Z/Data/Modules/forge-cli/Cargo.toml -- walk_skill_dir_flattens -v
```

Expected: PASS

- [ ] **Step 3: Run all tests**

```sh
TMPDIR=/private/tmp/claude-501 cargo test --manifest-path /Users/N4M3Z/Data/Modules/forge-cli/Cargo.toml
```

Expected: All pass (no regressions — existing skills without `user/` are unaffected)

- [ ] **Step 4: Run clippy and fmt**

```sh
cargo fmt --manifest-path /Users/N4M3Z/Data/Modules/forge-cli/Cargo.toml
cargo clippy --manifest-path /Users/N4M3Z/Data/Modules/forge-cli/Cargo.toml -- -D warnings
```

- [ ] **Step 5: Commit**

```sh
git add src/cli/assemble/sources.rs
git commit -m "fix: flatten skill user/ subdirectories during assembly"
```

---

### Task 4: Verify end-to-end with forge-core

- [ ] **Step 1: Build forge-cli**

```sh
cd /Users/N4M3Z/Data/Modules/forge-cli && make build
```

- [ ] **Step 2: Run forge assemble on forge-core**

```sh
forge assemble /Users/N4M3Z/Data/Modules/forge-core
```

Check output for flatten/override messages from ArchitectureDecision skill.

- [ ] **Step 3: Verify flattened files in build/**

```sh
ls /Users/N4M3Z/Data/Modules/forge-core/build/claude/skills/ArchitectureDecision/
```

Expected: `SKILL.md`, `TemplateReference.md`, `SchemaValidation.md`, `Example.md`, `ForgeADR.md`, `ContextKeeper.md` — all flat, no `user/` subdirectory.

- [ ] **Step 4: Run forge install on forge-core**

```sh
forge install /Users/N4M3Z/Data/Modules/forge-core
```

- [ ] **Step 5: Verify deployed files**

```sh
ls ~/.claude/skills/ArchitectureDecision/
```

Expected: Same flat listing — ForgeADR.md and ContextKeeper.md deployed alongside root companions.

- [ ] **Step 6: Commit and push**

```sh
cd /Users/N4M3Z/Data/Modules/forge-cli
git push
```
