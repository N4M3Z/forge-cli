use commands::manifest;
use commands::result::ActionResult;
use std::path::Path;

use super::templates::InitTemplates;

pub fn check_template_drift(module_root: &Path, result: &mut ActionResult) {
    let manifest_path = module_root.join(".manifest");
    if !manifest_path.is_file() {
        println!("  MISSING .manifest — run `forge init` to establish baseline");
        result
            .warnings
            .push(".manifest: missing — run forge init to establish baseline".to_string());
        return;
    }

    let manifest_content = match std::fs::read_to_string(&manifest_path) {
        Ok(content) => content,
        Err(error) => {
            result
                .warnings
                .push(format!(".manifest: cannot read: {error}"));
            return;
        }
    };

    let entries = match manifest::read(&manifest_content) {
        Ok(entries) => entries,
        Err(error) => {
            result
                .warnings
                .push(format!(".manifest: invalid format: {error}"));
            return;
        }
    };

    let module_name = resolve_module_name(module_root);

    for filename in entries.keys() {
        let target_path = module_root.join(filename);
        if !target_path.is_file() {
            println!("  MISSING {filename} (tracked in .manifest)");
            result
                .warnings
                .push(format!("{filename}: tracked in manifest but missing"));
            continue;
        }

        let Some(expected_hash) = template_hash(filename, &module_name) else {
            println!("  ok {filename}");
            continue;
        };

        let Ok(content) = std::fs::read_to_string(&target_path) else {
            result
                .warnings
                .push(format!("{filename}: cannot read file"));
            continue;
        };

        let actual_hash = manifest::content_sha256(&content);
        if actual_hash == expected_hash {
            println!("  ok {filename}");
        } else {
            println!("  DRIFT {filename}");
            result
                .warnings
                .push(format!("{filename}: drifted from current template"));
        }
    }
}

fn template_hash(filename: &str, module_name: &str) -> Option<String> {
    let data = InitTemplates::get(filename)?;
    let template_content = std::str::from_utf8(data.data.as_ref()).ok()?;
    let content = template_content.replace("${MODULE_NAME}", module_name);
    Some(manifest::content_sha256(&content))
}

fn resolve_module_name(module_root: &Path) -> String {
    module_root
        .canonicalize()
        .unwrap_or_else(|_| module_root.to_path_buf())
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests;
