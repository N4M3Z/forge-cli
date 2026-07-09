use super::*;

fn write_module(root: &Path, module_name: &str) {
    std::fs::write(
        root.join("module.yaml"),
        format!("name: {module_name}\nversion: 0.1.0\ndescription: test\nevents: []\n"),
    )
    .expect("module");
}

fn write_skill(root: &Path, name: &str, description: &str, body: &str) {
    let skill_dir = root.join("skills").join(name);
    std::fs::create_dir_all(&skill_dir).expect("skill dir");
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n\n{body}\n"),
    )
    .expect("skill");
}

fn write_rule(root: &Path, name: &str, description: &str) {
    let rules_dir = root.join("rules");
    std::fs::create_dir_all(&rules_dir).expect("rules dir");
    std::fs::write(
        rules_dir.join(format!("{name}.md")),
        format!("---\nname: {name}\ndescription: {description}\n---\n\nRule body.\n"),
    )
    .expect("rule");
}

#[test]
fn find_matches_trigger_word_across_modules() {
    let first = tempfile::tempdir().expect("first");
    let second = tempfile::tempdir().expect("second");
    write_module(first.path(), "first-module");
    write_module(second.path(), "second-module");
    write_skill(
        first.path(),
        "AlphaSkill",
        "General helper",
        "## USE-WHEN\nUse when handling invoices.",
    );
    write_skill(
        second.path(),
        "BetaSkill",
        "General helper",
        "## USE-WHEN\nUse when handling ledgers.",
    );

    let modules = vec![first.path().to_path_buf(), second.path().to_path_buf()];
    let results = search_modules(&modules, "ledgers", Some(KindFilter::Skills));

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "BetaSkill");
    assert_eq!(results[0].module, "second-module");
    assert_eq!(results[0].path, "skills/BetaSkill/SKILL.md");
    assert_eq!(results[0].source, "local");
}

#[test]
fn kind_filter_excludes_other_kinds() {
    let module = tempfile::tempdir().expect("module");
    write_module(module.path(), "fixture");
    write_skill(
        module.path(),
        "InvoiceSkill",
        "invoice workflow",
        "Use when handling invoices.",
    );
    write_rule(module.path(), "InvoiceRule", "invoice workflow");

    let modules = vec![module.path().to_path_buf()];
    let results = search_modules(&modules, "invoice", Some(KindFilter::Rules));

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "InvoiceRule");
    assert_eq!(results[0].kind, "rules");
}

#[test]
fn json_shape_contains_expected_fields() {
    let module = tempfile::tempdir().expect("module");
    write_module(module.path(), "fixture");
    write_skill(
        module.path(),
        "JsonSkill",
        "json result fixture",
        "Use when testing json output.",
    );

    let modules = vec![module.path().to_path_buf()];
    let results = search_modules(&modules, "json", Some(KindFilter::Skills));
    let value: serde_json::Value = serde_json::to_value(&results).expect("json");

    assert_eq!(value[0]["name"], "JsonSkill");
    assert_eq!(value[0]["kind"], "skills");
    assert_eq!(value[0]["module"], "fixture");
    assert_eq!(value[0]["path"], "skills/JsonSkill/SKILL.md");
    assert_eq!(value[0]["description"], "json result fixture");
    assert_eq!(value[0]["source"], "local");
    assert!(value[0]["score"].as_f64().expect("score") > 0.0);
}

#[test]
fn deterministic_order_uses_name_tiebreak() {
    let module = tempfile::tempdir().expect("module");
    write_module(module.path(), "fixture");
    write_skill(
        module.path(),
        "ZuluSkill",
        "shared keyword",
        "Body without extra match.",
    );
    write_skill(
        module.path(),
        "AlphaSkill",
        "shared keyword",
        "Body without extra match.",
    );

    let modules = vec![module.path().to_path_buf()];
    let results = search_modules(&modules, "keyword", Some(KindFilter::Skills));

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].name, "AlphaSkill");
    assert_eq!(results[1].name, "ZuluSkill");
    assert!((results[0].score - results[1].score).abs() < f64::EPSILON);
}
