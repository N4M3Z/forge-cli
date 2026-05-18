use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let script_path = manifest_dir.join("scripts").join("validate.sh");

    println!("cargo:rerun-if-changed=scripts/validate.sh");

    let bytes = fs::read(&script_path).unwrap_or_else(|error| {
        panic!("build.rs: read {}: {error}", script_path.display());
    });
    let hex = format!("{:x}", Sha256::digest(&bytes));

    println!("cargo:rustc-env=VALIDATE_SH_SHA={hex}");
}
