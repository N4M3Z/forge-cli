use commands::result::{ActionResult, SkipReason, SkippedFile};
use console::Style;
use std::collections::BTreeMap;

pub fn print(result: &ActionResult, json_output: bool, verb: &str) {
    if json_output {
        match serde_json::to_string_pretty(result) {
            Ok(json) => println!("{json}"),
            Err(err) => eprintln!("failed to serialize result: {err}"),
        }
        return;
    }

    let grouped = group_by_provider(result);

    println!();
    print_providers(&grouped, result);
    print_errors(result);
    print_summary(result, verb);
    println!();
}

struct ProviderGroup<'a> {
    kinds: BTreeMap<&'a str, usize>,
    skips: Vec<&'a SkippedFile>,
}

fn group_by_provider(result: &ActionResult) -> BTreeMap<&str, ProviderGroup<'_>> {
    let mut groups: BTreeMap<&str, ProviderGroup<'_>> = BTreeMap::new();

    for entry in &result.installed {
        let kind = extract_content_kind(&entry.target);
        *groups
            .entry(&entry.provider)
            .or_insert_with(|| ProviderGroup {
                kinds: BTreeMap::new(),
                skips: Vec::new(),
            })
            .kinds
            .entry(kind)
            .or_default() += 1;
    }

    for skipped in &result.skipped {
        groups
            .entry(&skipped.provider)
            .or_insert_with(|| ProviderGroup {
                kinds: BTreeMap::new(),
                skips: Vec::new(),
            })
            .skips
            .push(skipped);
    }

    groups
}

fn print_providers(groups: &BTreeMap<&str, ProviderGroup<'_>>, result: &ActionResult) {
    let green = Style::new().green();
    let red = Style::new().red();
    let yellow = Style::new().yellow();
    let dim = Style::new().dim();
    let bold = Style::new().bold();

    for (provider, group) in groups {
        let has_errors = result
            .errors
            .iter()
            .any(|error| error.contains(&format!("({provider})")));

        let symbol = if has_errors {
            red.apply_to("✗")
        } else {
            green.apply_to("✓")
        };

        println!(" {} {}", symbol, bold.apply_to(provider));

        if !group.kinds.is_empty() {
            let parts: Vec<String> = group
                .kinds
                .iter()
                .map(|(kind, count)| format!("{} {}", dim.apply_to(kind), count))
                .collect();
            println!("   {}", parts.join("  "));
        }

        for skipped in &group.skips {
            let relative = extract_relative_path(&skipped.target);
            let reason = match &skipped.reason {
                SkipReason::UserModified => "user modified",
                SkipReason::Unchanged => "unchanged",
                SkipReason::TargetMismatch => "target mismatch",
            };
            println!(
                "   {} {} {} {}",
                yellow.apply_to("○"),
                dim.apply_to(relative),
                dim.apply_to("—"),
                yellow.apply_to(reason)
            );
        }
    }
}

fn print_errors(result: &ActionResult) {
    let red = Style::new().red();
    for error in &result.errors {
        println!("   {} {}", red.apply_to("✗"), red.apply_to(error));
    }
}

fn print_summary(result: &ActionResult, verb: &str) {
    let green = Style::new().green();
    let yellow = Style::new().yellow();
    let red = Style::new().red();

    let action_count = result.installed.len();
    let skipped_count = result.skipped.len();
    let error_count = result.errors.len();

    if action_count == 0 && skipped_count == 0 && error_count == 0 {
        return;
    }

    println!();
    let mut parts: Vec<String> = Vec::new();
    if action_count > 0 {
        parts.push(format!("{} {} {}", green.apply_to("●"), action_count, verb));
    }
    if skipped_count > 0 {
        parts.push(format!(
            "{} {} skipped",
            yellow.apply_to("○"),
            skipped_count
        ));
    }
    if error_count > 0 {
        parts.push(format!(
            "{} {} {}",
            red.apply_to("✗"),
            error_count,
            if error_count == 1 { "error" } else { "errors" }
        ));
    }
    println!(" {}", parts.join("  "));
}

fn extract_content_kind(path: &str) -> &str {
    for kind in &["agents", "skills", "rules"] {
        if path.contains(&format!("/{kind}/")) {
            return kind;
        }
    }
    "files"
}

fn extract_relative_path(path: &str) -> &str {
    let segments: Vec<&str> = path.rsplit('/').take(3).collect();
    let start =
        path.len() - segments.iter().map(|string| string.len() + 1).sum::<usize>() + 1;
    if start < path.len() {
        &path[start..]
    } else {
        path
    }
}
