//! Drift guard: the SHA-256 baked into the `forge` binary by `build.rs` must
//! match the actual on-disk `scripts/validate.sh`. If `validate.sh` changes
//! and `cargo build` re-runs, the new const is correct by construction. This
//! test catches the case where `build.rs` is wired to the wrong path or the
//! const is bypassed.

use sha2::{Digest, Sha256};

const VALIDATE_SH: &[u8] = include_bytes!("../scripts/validate.sh");

#[test]
fn embedded_validate_sh_sha_matches_script() {
    let actual = format!("{:x}", Sha256::digest(VALIDATE_SH));
    assert_eq!(
        actual,
        commands::VALIDATE_SH_SHA,
        "VALIDATE_SH_SHA out of sync with scripts/validate.sh — rebuild forge-cli"
    );
}
