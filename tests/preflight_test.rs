mod common;

use common::TestEnv;
use predicates::prelude::*;

fn failing_school() -> TestEnv {
    let env = TestEnv::new();
    env.write_dogfood_school("name = \"test-school\"\nbackend = \"flaude\"\n[[mcp]]\nname = \"sample\"\nurl = \"https://example.com/mcp\"\n");
    env.write_file("skills/fixture/SKILL.md", "# Fixture skill\n");
    env
}

#[test]
fn setup_checks_school_backend_before_publishing_project() {
    let env = TestEnv::new();
    env.setup_embedded_school("fixture-school");
    env.write_file(
        "school.toml",
        "name = \"fixture-school\"\nbackend = \"flaude\"\n",
    );

    env.ace()
        .env(
            "FLAUDE_PREFLIGHT",
            r#"{"unavailable":"install the fixture prerequisite"}"#,
        )
        .args(["setup", "."])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("install the fixture prerequisite"));

    env.assert_not_exists("ace.toml");
    env.assert_not_exists(".claude");
    env.assert_not_exists(".gitignore");
    env.assert_not_exists(".flaude-mcp-records.jsonl");
    env.assert_not_exists(".flaude-exec-records.jsonl");
}

#[test]
fn failed_preflight_preserves_exclusions_before_registering() {
    let env = failing_school();
    let original = "# retain my choice\nexclude_mcp = [\"sample\"]\n";
    env.write_file("ace.local.toml", original);

    env.ace()
        .env(
            "FLAUDE_PREFLIGHT",
            r#"{"failed":"cannot inspect fixture installation"}"#,
        )
        .args(["mcp", "register", "sample"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(
            "cannot inspect fixture installation",
        ));

    assert_eq!(env.read_file("ace.local.toml"), original);
    env.assert_not_exists(".flaude-mcp-records.jsonl");
}

#[test]
fn prepare_fails_before_linking_gitignore_registration_or_launch() {
    for (response, code) in [
        (r#"{"unavailable":"fixture dependency missing"}"#, 2),
        (r#"{"failed":"fixture probe failed"}"#, 3),
        (r#"{"invalid_arguments":"fixture arguments invalid"}"#, 1),
    ] {
        let env = failing_school();
        let original = env.read_file("ace.toml");

        env.ace()
            .env("FLAUDE_PREFLIGHT", response)
            .args(["-p", "hello"])
            .assert()
            .code(code);

        assert_eq!(env.read_file("ace.toml"), original);
        env.assert_not_exists(".claude");
        env.assert_not_exists(".gitignore");
        env.assert_not_exists(".flaude-mcp-records.jsonl");
        env.assert_not_exists(".flaude-exec-records.jsonl");
    }
}

#[test]
fn read_only_configuration_does_not_run_preflight() {
    let env = failing_school();

    env.ace()
        .env("FLAUDE_PREFLIGHT", r#"{"unavailable":"not installed"}"#)
        .args(["config", "get", "backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("flaude"));

    env.assert_not_exists(".flaude-calls.jsonl");
}

#[test]
fn malformed_fixture_control_cannot_silently_pass_preflight() {
    let env = failing_school();

    env.ace()
        .env("FLAUDE_PREFLIGHT", "not JSON")
        .args(["-p", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("FLAUDE_PREFLIGHT"));

    env.assert_not_exists(".claude");
    env.assert_not_exists(".flaude-exec-records.jsonl");
}

#[test]
fn startup_verifies_resolved_instance_before_registering_and_launching() {
    let env = failing_school();
    env.write_file("ace.toml", "school = \".\"\nbackend = \"custom\"\nenv = { CONTEXT = \"global\" }\n[backends.custom]\nkind = \"flaude\"\ncmd = [\"wrapper\", \"prefix\"]\nenv = { CONTEXT = \"backend\", FLAUDE_PREFLIGHT = '\"ready\"' }\nmodel = \"configured-model\"\neffort = \"low\"\n");
    let prompt = "@secret-file\n--not-an-option";
    env.write_file("secret-file", "must not be read");

    env.ace()
        .env(
            "FLAUDE_PREFLIGHT",
            r#"{"unavailable":"ambient value must lose"}"#,
        )
        .args(["-p", prompt])
        .assert()
        .success();

    let calls = env.read_file(".flaude-calls.jsonl");
    let calls: Vec<serde_json::Value> = calls
        .lines()
        .map(|line| serde_json::from_str(line).expect("fixture call record"))
        .collect();
    let actions: Vec<_> = calls
        .iter()
        .map(|call| call["action"].as_str().expect("action"))
        .collect();
    assert_eq!(actions.first(), Some(&"preflight"));
    assert!(
        actions.contains(&"mcp_add"),
        "startup must provision the missing server"
    );
    assert_eq!(calls[0]["cmd"], serde_json::json!(["wrapper", "prefix"]));
    assert_eq!(calls[0]["model"], "configured-model");
    assert_eq!(calls[0]["effort"], "low");
    assert_eq!(calls[0]["env"]["CONTEXT"], "backend");
    assert_eq!(
        calls[0]["project_dir"],
        env.root().to_string_lossy().as_ref()
    );
    let records = env.read_flaude_one_shot_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].prompt_text.as_deref(), Some(prompt));
    assert_eq!(
        records[0].env.get("CONTEXT").map(String::as_str),
        Some("backend")
    );
    assert_eq!(records[0].cmd, ["wrapper", "prefix"]);
    env.assert_symlink(".claude/skills/fixture", "skills/fixture");
}

#[test]
fn startup_skips_mcp_inventory_when_no_school_server_is_effective() {
    for (servers, exclusions) in [
        ("", ""),
        (
            "[[mcp]]\nname = \"sample\"\nurl = \"https://example.com/mcp\"\n",
            "exclude_mcp = [\"sample\"]\n",
        ),
    ] {
        let env = TestEnv::new();
        env.write_dogfood_school(&format!(
            "name = \"test-school\"\nbackend = \"flaude\"\n{servers}"
        ));
        env.write_file("skills/fixture/SKILL.md", "# Fixture skill\n");
        env.write_file("ace.local.toml", exclusions);
        env.mkdir(".flaude-mcp-list");

        env.ace().args(["-p", "hello"]).assert().success();

        env.assert_symlink(".claude/skills/fixture", "skills/fixture");
        assert_eq!(env.read_flaude_one_shot_records().len(), 1);
        env.assert_not_exists(".flaude-mcp-records.jsonl");
    }
}

#[test]
fn setup_skips_mcp_inventory_when_no_prospective_server_is_effective() {
    for (servers, exclusions) in [
        ("", ""),
        (
            "[[mcp]]\nname = \"sample\"\nurl = \"https://example.com/mcp\"\n",
            "exclude_mcp = [\"sample\"]\n",
        ),
    ] {
        let env = TestEnv::new();
        env.setup_embedded_school("fixture-school");
        env.write_file(
            "school.toml",
            &format!("name = \"fixture-school\"\nbackend = \"flaude\"\n{servers}"),
        );
        env.write_file("ace.local.toml", exclusions);
        env.mkdir(".flaude-mcp-list");

        env.ace().args(["setup", "."]).assert().success();

        env.assert_symlink(".claude/skills/maverick", "skills/maverick");
        env.assert_exists("ace.toml");
        env.assert_not_exists(".flaude-mcp-records.jsonl");
    }
}

#[test]
fn failed_inventory_stops_startup_before_project_mutations() {
    let env = failing_school();
    let original = env.read_file("ace.toml");
    env.mkdir(".flaude-mcp-list");

    env.ace()
        .args(["-p", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(".flaude-mcp-list"));

    assert_eq!(env.read_file("ace.toml"), original);
    env.assert_not_exists(".claude");
    env.assert_not_exists(".gitignore");
    env.assert_not_exists(".flaude-mcp-records.jsonl");
    env.assert_not_exists(".flaude-exec-records.jsonl");
}

#[test]
fn failed_prospective_inventory_stops_setup_before_config_publication() {
    let env = TestEnv::new();
    env.setup_embedded_school("fixture-school");
    env.write_file("school.toml", "name = \"fixture-school\"\nbackend = \"flaude\"\n[[mcp]]\nname = \"sample\"\nurl = \"https://example.com/mcp\"\n");
    env.mkdir(".flaude-mcp-list");

    env.ace()
        .args(["setup", "."])
        .assert()
        .code(3)
        .stderr(predicate::str::contains(".flaude-mcp-list"));

    env.assert_not_exists("ace.toml");
    env.assert_not_exists(".claude");
    env.assert_not_exists(".gitignore");
    env.assert_not_exists(".flaude-mcp-records.jsonl");
}
