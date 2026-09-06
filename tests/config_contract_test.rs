mod common;

use common::TestEnv;
use predicates::prelude::*;

#[test]
fn inspection_reports_configured_selection_policies_without_a_school() {
    let env = TestEnv::new();
    env.write_file(
        "config/ace/ace.toml",
        "skills = [\"user-*\"]\ninclude_skills = [\"a\", \"b\"]\nexclude_mcp = [\"one\"]\n",
    );
    env.write_file(
        "ace.toml",
        "skills = [\"project-*\"]\ninclude_skills = [\"b\", \"c\"]\nexclude_skills = [\"x\"]\n",
    );
    env.write_file(
        "ace.local.toml",
        "skills = []\nexclude_mcp = [\"one\", \"two\"]\n",
    );
    let output = env
        .ace()
        .arg("config")
        .assert()
        .success()
        .get_output()
        .clone();
    let document: toml::Value = toml::from_str(std::str::from_utf8(&output.stdout).expect("UTF-8"))
        .expect("effective TOML");
    assert_eq!(
        document["skills"].as_array().expect("skills")[0].as_str(),
        Some("project-*")
    );
    assert_eq!(
        document["include_skills"]
            .as_array()
            .expect("includes")
            .len(),
        3
    );
    assert_eq!(
        document["exclude_mcp"]
            .as_array()
            .expect("MCP exclusions")
            .len(),
        2
    );
    env.ace()
        .args(["config", "get", "include_skills"])
        .assert()
        .success()
        .stdout("[\"a\", \"b\", \"c\"]\n");
    env.ace()
        .args(["config", "set", "skills", "anything"])
        .assert()
        .failure();
}

#[test]
fn explanation_marks_ignored_personal_fields_and_preserves_boolean_types() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "resume = false\n");
    env.write_file("ace.local.toml", "resume = true\n");
    env.ace()
        .args(["config", "explain", "resume"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "project:  false  (ignored: personal-only)",
        ))
        .stdout(predicate::str::contains("local:    true  ← winner"));
}

#[test]
fn explanation_quotes_literal_keys_and_control_characters() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "backend = \"codex\"\n");
    let instance = "team.codex\nrow";
    let key = format!("backends.{instance}.effort");
    env.ace()
        .args(["config", "set", &key, "high"])
        .assert()
        .success();
    let output = env
        .ace()
        .args(["config", "explain", &key])
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(output.stdout).expect("UTF-8");
    let first = stdout.lines().next().expect("winner line");
    assert_eq!(
        first,
        r#"backends."team.codex\nrow".effort = "high"  [project]"#
    );
    assert_eq!(
        stdout.lines().count(),
        6,
        "a key must not create extra rows"
    );
}

#[test]
fn saved_write_reports_a_subsequent_inspection_failure_honestly() {
    let env = TestEnv::new();
    env.write_file("school.toml", "not valid [ toml");
    env.ace()
        .args(["config", "set", "school", "."])
        .assert()
        .success()
        .stderr(predicate::str::contains("saved"))
        .stderr(predicate::str::contains("could not be inspected"));
    env.assert_contains("ace.toml", "school = \".\"");
}

#[test]
fn absent_and_explicit_empty_backend_values_retain_their_provenance() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "[backends.codex]\nmodel = \"configured\"\n");
    let output = env
        .ace()
        .arg("config")
        .assert()
        .success()
        .get_output()
        .clone();
    let document: toml::Value =
        toml::from_str(std::str::from_utf8(&output.stdout).expect("UTF-8")).expect("TOML");
    assert!(document["backends"]["codex"].get("effort").is_none());
    env.ace()
        .args(["config", "explain", "backends.codex.effort"])
        .assert()
        .success()
        .stdout("backends.codex.effort = \"\"  [default]\n");
    env.ace()
        .args(["--local", "config", "set", "backends.codex.effort", ""])
        .assert()
        .success();
    env.ace()
        .args(["config", "explain", "backends.codex.effort"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "backends.codex.effort = \"\"  [local]",
        ));
    let output = env
        .ace()
        .arg("config")
        .assert()
        .success()
        .get_output()
        .clone();
    let document: toml::Value =
        toml::from_str(std::str::from_utf8(&output.stdout).expect("UTF-8")).expect("TOML");
    assert_eq!(document["backends"]["codex"]["effort"].as_str(), Some(""));
}

#[test]
fn explicit_local_default_overrides_inherited_yolo() {
    let env = TestEnv::new();
    env.write_file("config/ace/ace.toml", "trust = \"yolo\"\n");

    env.ace()
        .args(["config", "set", "trust", "default"])
        .assert()
        .success();

    env.assert_contains("ace.local.toml", "trust = \"default\"");
    env.ace()
        .args(["config", "get", "trust"])
        .assert()
        .success()
        .stdout("default\n");
    env.ace()
        .args(["config", "explain", "trust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("trust = \"default\"  [local]"));
}

#[test]
fn explicit_cli_default_overrides_local_yolo() {
    let env = TestEnv::new();
    env.write_file("ace.local.toml", "trust = \"yolo\"\n");

    env.ace()
        .args(["--trust", "default", "config", "get", "trust"])
        .assert()
        .success()
        .stdout("default\n");
    env.ace()
        .args(["--trust", "default", "config", "explain", "trust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("trust = \"default\"  [override]"));
    env.assert_contains("ace.local.toml", "trust = \"yolo\"");
}

#[test]
fn explicit_trust_default_takes_precedence_over_legacy_yolo_in_same_layer() {
    let env = TestEnv::new();
    env.write_file("ace.local.toml", "trust = \"default\"\nyolo = true\n");

    env.ace()
        .args(["config", "get", "trust"])
        .assert()
        .success()
        .stdout("default\n");
}

#[test]
fn setting_one_field_preserves_unknown_content_and_comments() {
    let env = TestEnv::new();
    env.write_file(
        "ace.toml",
        "# shared settings\nbackend = \"claude\" # chosen backend\neffort = \"high\"\n\
         [future]\nmode = \"custom\" # retain this\n",
    );

    env.ace()
        .args(["config", "set", "backend", "codex"])
        .assert()
        .success();

    env.assert_contains("ace.toml", "# shared settings");
    env.assert_contains("ace.toml", "# chosen backend");
    env.assert_contains("ace.toml", "effort = \"high\"");
    env.assert_contains("ace.toml", "mode = \"custom\" # retain this");
    env.ace()
        .args(["config", "get", "backend"])
        .assert()
        .success()
        .stdout("codex\n");
}

#[cfg(unix)]
#[test]
fn config_set_follows_a_relative_symlink_to_a_missing_target() {
    let env = TestEnv::new();
    std::os::unix::fs::symlink("personal/settings.toml", env.path("ace.local.toml"))
        .expect("create relative config link");

    env.ace()
        .args(["config", "set", "trust", "default"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_link(env.path("ace.local.toml")).expect("retained link"),
        std::path::Path::new("personal/settings.toml")
    );
    env.assert_contains("personal/settings.toml", "trust = \"default\"");
}

#[test]
fn config_set_preserves_quoted_key_formatting_in_regular_dotted_and_inline_tables() {
    let documents = [
        "[env]\n# keep key explanation\n'TEAM.KEY'  =  \"old\" # keep trailing comment\n",
        "# keep dotted explanation\nenv . 'TEAM.KEY'  =  \"old\" # keep trailing comment\n",
        "env = {\n# keep inline explanation\n'TEAM.KEY'  =  \"old\", # keep trailing comment\n}\n",
    ];
    for original in documents {
        let env = TestEnv::new();
        env.write_file("ace.toml", original);

        env.ace()
            .args(["config", "set", "env.TEAM.KEY", "new"])
            .assert()
            .success();

        assert_eq!(
            env.read_file("ace.toml"),
            original.replace("\"old\"", "\"new\"")
        );
    }
}

#[test]
fn project_backend_and_local_effort_agree_across_inspection_commands() {
    let env = TestEnv::new();
    env.write_file(
        "ace.toml",
        "backend = \"codex\"\n[backends.codex]\nmodel = \"provider/model\"\neffort = \"low\"\n",
    );
    env.write_file("ace.local.toml", "[backends.codex]\neffort = \"high\"\n");

    env.ace()
        .args(["config", "get", "backend"])
        .assert()
        .success()
        .stdout("codex\n");
    env.ace()
        .args(["config", "get", "backends.codex.effort"])
        .assert()
        .success()
        .stdout("high\n");
    env.ace()
        .args(["config", "get", "backends.codex.model"])
        .assert()
        .success()
        .stdout("provider/model\n");
    env.ace()
        .args(["config", "explain", "backends.codex.effort"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "backends.codex.effort = \"high\"  [local]",
        ))
        .stdout(predicate::str::contains("\"low\""));

    let output = env
        .ace()
        .arg("config")
        .assert()
        .success()
        .get_output()
        .clone();
    let stdout = String::from_utf8(output.stdout).expect("config output UTF-8");
    let shown: toml::Value = toml::from_str(&stdout).expect("effective TOML");
    assert_eq!(shown["backend"].as_str(), Some("codex"));
    assert_eq!(
        shown["backends"]["codex"]["model"].as_str(),
        Some("provider/model")
    );
    assert_eq!(shown["backends"]["codex"]["effort"].as_str(), Some("high"));
}

#[test]
fn dotted_custom_backend_fields_remain_inspectable_without_a_kind() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "backend = \"team.codex\"\n");

    env.ace()
        .args([
            "--local",
            "config",
            "set",
            "backends.team.codex.effort",
            "ultra",
        ])
        .assert()
        .success();
    env.ace()
        .args(["config", "get", "backends.team.codex.effort"])
        .assert()
        .success()
        .stdout("ultra\n");
    env.ace()
        .args(["config", "explain", "backends.team.codex.effort"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"ultra\"  [local]"));
}

#[test]
fn builtin_backend_can_be_set_without_preexisting_configuration() {
    let env = TestEnv::new();

    env.ace()
        .args(["config", "set", "backend", "codex"])
        .assert()
        .success();

    env.assert_contains("ace.toml", "backend = \"codex\"");
    env.ace()
        .args(["config", "get", "backend"])
        .assert()
        .success()
        .stdout("codex\n");
}

#[test]
fn explanation_escapes_string_values_without_creating_extra_lines() {
    let env = TestEnv::new();
    let prompt = "say \"hi\"\npath\\file\tend";
    env.write_file("ace.toml", "backend = \"codex\"\n");

    let output = env
        .ace()
        .args([
            "--session-prompt",
            prompt,
            "config",
            "explain",
            "session_prompt",
        ])
        .assert()
        .success()
        .get_output()
        .clone();

    let stdout = String::from_utf8(output.stdout).expect("explain output UTF-8");
    let winner = stdout.lines().next().expect("winner line");
    assert_eq!(
        winner,
        r#"session_prompt = "say \"hi\"\npath\\file\tend"  [override]"#
    );
    assert_eq!(
        stdout.lines().count(),
        6,
        "a value must not create extra rows"
    );
    assert!(stdout.contains("\\n"), "newline must be escaped: {stdout}");
    assert!(stdout.contains("\\t"), "tab must be escaped: {stdout}");
}

#[test]
fn ignored_project_resume_write_reports_its_effective_consequence() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "backend = \"codex\"\n");

    env.ace()
        .args(["--project", "config", "set", "resume", "false"])
        .assert()
        .success()
        .stderr(predicate::str::contains("project"))
        .stderr(predicate::str::contains("ignor"));

    env.assert_contains("ace.toml", "resume = false");
    env.ace()
        .args(["config", "get", "resume"])
        .assert()
        .success()
        .stdout("true\n");
}

#[test]
fn overridden_backend_write_reports_local_winner() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "backend = \"claude\"\n");
    env.write_file("ace.local.toml", "backend = \"opencode\"\n");

    env.ace()
        .args(["config", "set", "backend", "codex"])
        .assert()
        .success()
        .stderr(predicate::str::contains("local"))
        .stderr(predicate::str::contains("opencode"))
        .stderr(predicate::str::contains("overrid").or(predicate::str::contains("effective")));

    env.assert_contains("ace.toml", "backend = \"codex\"");
    env.ace()
        .args(["config", "get", "backend"])
        .assert()
        .success()
        .stdout("opencode\n");
}
