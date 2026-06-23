use super::*;

#[test]
fn resolve_passes_through_absolute_paths() {
    assert_eq!(resolve("/abs/path"), Some(PathBuf::from("/abs/path")));
}

#[test]
fn resolve_expands_leading_tilde() {
    let home = dirs::home_dir().expect("home dir for test");
    assert_eq!(resolve("~/forge"), Some(home.join("forge")));
}

#[test]
fn parses_path_locations_from_yaml() {
    let config: WatchlistConfig =
        serde_yaml::from_str("locations:\n    - /a\n    - /b\n").expect("parse yaml");
    assert_eq!(
        config.locations,
        vec![
            WatchEntry::Path("/a".to_string()),
            WatchEntry::Path("/b".to_string()),
        ]
    );
}

#[test]
fn parses_mixed_path_and_git_entries() {
    let yaml = "locations:\n    - /local/repo\n    - git: https://github.com/N4M3Z/forge-core\n      ref: 0123456789abcdef0123456789abcdef01234567\n";
    let config: WatchlistConfig = serde_yaml::from_str(yaml).expect("parse mixed yaml");
    assert_eq!(
        config.locations,
        vec![
            WatchEntry::Path("/local/repo".to_string()),
            WatchEntry::Git {
                git: "https://github.com/N4M3Z/forge-core".to_string(),
                reference: "0123456789abcdef0123456789abcdef01234567".to_string(),
            },
        ]
    );
}

#[test]
fn git_entry_round_trips_through_yaml() {
    let entry = WatchEntry::Git {
        git: "https://github.com/N4M3Z/forge-core".to_string(),
        reference: "0123456789abcdef0123456789abcdef01234567".to_string(),
    };
    let config = WatchlistConfig {
        locations: vec![entry.clone()],
    };
    let yaml = serde_yaml::to_string(&config).expect("serialize");
    let parsed: WatchlistConfig = serde_yaml::from_str(&yaml).expect("reparse");
    assert_eq!(parsed.locations, vec![entry]);
}

#[test]
fn empty_yaml_yields_no_locations() {
    let config: WatchlistConfig = serde_yaml::from_str("{}").expect("parse empty");
    assert!(config.locations.is_empty());
}

#[test]
fn add_git_rejects_non_https_url() {
    let error = add_git("git@github.com:N4M3Z/forge-core.git", "0123", false)
        .expect_err("ssh url must be rejected");
    assert!(error.contains("https://"));
}

#[test]
fn add_git_rejects_short_sha() {
    let error = add_git("https://github.com/N4M3Z/forge-core", "abc123", false)
        .expect_err("short sha must be rejected");
    assert!(error.contains("40-char"), "got: {error}");
}

#[test]
fn add_git_rejects_non_hex_sha() {
    let bad = "z123456789abcdef0123456789abcdef01234567";
    let error = add_git("https://github.com/N4M3Z/forge-core", bad, false)
        .expect_err("non-hex sha must be rejected");
    assert!(error.contains("hex"), "got: {error}");
}

#[test]
fn add_git_rejects_uppercase_sha() {
    let upper = "0123456789ABCDEF0123456789ABCDEF01234567";
    let error = add_git("https://github.com/N4M3Z/forge-core", upper, false)
        .expect_err("uppercase sha must be rejected");
    assert!(error.contains("hex"), "got: {error}");
}

#[test]
fn add_git_rejects_userinfo_in_host() {
    let valid_sha = "0123456789abcdef0123456789abcdef01234567";
    let error = add_git("https://user@github.com/N4M3Z/forge-core", valid_sha, false)
        .expect_err("user@ in host must be rejected");
    assert!(error.contains("user@"), "got: {error}");
}

#[test]
fn add_git_rejects_empty_host() {
    let valid_sha = "0123456789abcdef0123456789abcdef01234567";
    let error = add_git("https:///N4M3Z/forge-core", valid_sha, false)
        .expect_err("empty host must be rejected");
    assert!(error.contains("no host"), "got: {error}");
}

#[test]
fn load_strict_from_absent_path_is_empty_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("watchlist.yaml");
    let config = load_strict_from(&path).expect("absent file is ok");
    assert!(config.locations.is_empty());
}

#[test]
fn load_strict_from_zero_byte_file_is_empty_ok() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("watchlist.yaml");
    std::fs::write(&path, "").expect("write empty");
    let config = load_strict_from(&path).expect("empty file is ok");
    assert!(config.locations.is_empty());
}

#[test]
fn load_strict_from_valid_file_returns_locations() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("watchlist.yaml");
    std::fs::write(&path, "locations:\n    - /a\n    - /b\n").expect("write valid");
    let config = load_strict_from(&path).expect("valid file parses");
    assert_eq!(
        config.locations,
        vec![
            WatchEntry::Path("/a".to_string()),
            WatchEntry::Path("/b".to_string()),
        ]
    );
}

#[test]
fn load_strict_from_corrupt_file_errs_without_touching_bytes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("watchlist.yaml");
    let corrupt = "locations:\n    - [unterminated\n";
    std::fs::write(&path, corrupt).expect("write corrupt");
    let error = load_strict_from(&path).expect_err("corrupt file must error");
    assert!(error.contains("malformed"), "got: {error}");
    let after = std::fs::read_to_string(&path).expect("reread");
    assert_eq!(after, corrupt, "corrupt file must be left untouched");
}

#[test]
fn load_strict_from_unknown_key_errs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("watchlist.yaml");
    std::fs::write(&path, "version: 1\nlocations: []\n").expect("write unknown key");
    let error = load_strict_from(&path).expect_err("unknown key must error");
    assert!(error.contains("malformed"), "got: {error}");
}

#[test]
fn load_lenient_from_corrupt_file_is_empty() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("watchlist.yaml");
    std::fs::write(&path, "locations:\n    - [unterminated\n").expect("write corrupt");
    let config = load_lenient_from(&path);
    assert!(config.locations.is_empty());
}

#[test]
fn json_label_with_backslash_and_newline_round_trips() {
    let label = WatchEntry::Path("a\\b\nc".to_string()).label();
    let value = serde_json::json!({ "locations": [label] });
    let parsed: serde_json::Value = serde_json::from_str(&value.to_string()).expect("valid JSON");
    assert_eq!(parsed["locations"][0], "a\\b\nc");
}

#[test]
fn json_announce_message_with_quote_and_newline_is_valid() {
    let message = "say \"hi\"\nthen bye";
    let value = serde_json::json!({ "message": message });
    let parsed: serde_json::Value = serde_json::from_str(&value.to_string()).expect("valid JSON");
    assert_eq!(parsed["message"], "say \"hi\"\nthen bye");
}
