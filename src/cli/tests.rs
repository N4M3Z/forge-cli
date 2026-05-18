//! Drift guard: every `forge install` / `forge validate` invocation in the
//! init templates must parse cleanly against the current clap definition. If
//! the CLI drops a positional or renames a flag and the templates lag, this
//! test fails on the changing PR rather than on consumer reports a month later.

use clap::Parser;

use super::Cli;

const TEMPLATE_MAKEFILE: &str = include_str!("../../templates/init/Makefile");
const TEMPLATE_PRE_COMMIT_CONFIG: &str =
    include_str!("../../templates/init/.pre-commit-config.yaml");

fn extract_forge_invocations(source: &str) -> Vec<Vec<&str>> {
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            let rest = trimmed
                .strip_prefix("$(FORGE) ")
                .or_else(|| trimmed.strip_prefix("entry: forge "))?;
            let argv: Vec<&str> = std::iter::once("forge")
                .chain(rest.split_whitespace())
                .collect();
            Some(argv)
        })
        .collect()
}

#[test]
fn every_makefile_forge_call_parses() {
    let invocations = extract_forge_invocations(TEMPLATE_MAKEFILE);
    assert!(
        !invocations.is_empty(),
        "Makefile should contain at least one forge invocation; extractor regression"
    );
    for argv in invocations {
        Cli::try_parse_from(&argv).unwrap_or_else(|error| {
            panic!("templates/init/Makefile invocation {argv:?} rejected by clap: {error}");
        });
    }
}

#[test]
fn every_pre_commit_config_forge_call_parses() {
    let invocations = extract_forge_invocations(TEMPLATE_PRE_COMMIT_CONFIG);
    assert!(
        !invocations.is_empty(),
        "pre-commit-config.yaml should contain at least one forge invocation"
    );
    for argv in invocations {
        Cli::try_parse_from(&argv).unwrap_or_else(|error| {
            panic!(
                "templates/init/.pre-commit-config.yaml invocation {argv:?} rejected by clap: {error}"
            );
        });
    }
}
