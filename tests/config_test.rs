mod common;

use common::TestEnv;

#[test]
fn config_shows_effective() {
    let env = TestEnv::new();
    env.setup_embedded("top-gun");

    let output = env.ace().args(["config"]).output().expect("ace config");

    assert!(output.status.success(), "ace config should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should contain the school specifier and backend field.
    assert!(
        stdout.contains("school"),
        "output should contain school field"
    );
    assert!(
        stdout.contains("backend"),
        "output should contain backend field"
    );
}

#[test]
fn config_includes_school_toml() {
    let env = TestEnv::new();
    env.setup_embedded("top-gun");

    let output = env.ace().args(["config"]).output().expect("ace config");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("# school.toml"),
        "should include school.toml section header"
    );
    assert!(
        stdout.contains("top-gun"),
        "should include school name from school.toml"
    );
}

#[test]
fn config_shows_trust_from_local() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.write_file("ace.local.toml", "trust = \"auto\"\n");

    let output = env.ace().args(["config"]).output().expect("ace config");

    assert!(output.status.success(), "ace config should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("trust = \"auto\""),
        "trust should appear in effective config"
    );
}

#[test]
fn config_backcompat_yolo_becomes_trust() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.write_file("ace.local.toml", "yolo = true\n");

    let output = env.ace().args(["config"]).output().expect("ace config");

    assert!(output.status.success(), "ace config should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("trust = \"yolo\""),
        "yolo=true should resolve to trust=yolo"
    );
}

#[test]
fn config_no_ace_toml() {
    let env = TestEnv::new();
    // No ace.toml — require_state should fail.

    env.ace().args(["config"]).assert().failure();
}

#[test]
fn config_backend_flag_overrides_effective_backend() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"phoenix\"\n");

    let output = env
        .ace()
        .args(["--backend", "codex", "config"])
        .output()
        .expect("ace config");

    assert!(output.status.success(), "ace config should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("backend = \"codex\""),
        "backend override should appear in effective config"
    );
}

#[test]
fn config_backend_short_flag_overrides_effective_backend() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"phoenix\"\n");

    let output = env
        .ace()
        .args(["-b", "codex", "config"])
        .output()
        .expect("ace config");

    assert!(output.status.success(), "ace config should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("backend = \"codex\""),
        "short backend override should appear in effective config"
    );
}

#[test]
fn config_backend_alias_flag_overrides_effective_backend() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"phoenix\"\n");

    let output = env
        .ace()
        .args(["--codex", "config"])
        .output()
        .expect("ace config");

    assert!(output.status.success(), "ace config should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("backend = \"codex\""),
        "backend alias should appear in effective config"
    );
}

#[test]
fn config_backend_alias_conflicts_with_backend_flag() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"phoenix\"\n");

    env.ace()
        .args(["--backend", "codex", "--claude", "config"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "cannot combine multiple backend override flags",
        ));
}

// -- config get --

#[test]
fn config_get_backend() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"phoenix\"\n");

    let output = env
        .ace()
        .args(["config", "get", "backend"])
        .output()
        .expect("ace config get backend");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "flaude");
}

#[test]
fn config_get_school() {
    let env = TestEnv::new();
    env.setup_embedded("top-gun");

    let output = env
        .ace()
        .args(["config", "get", "school"])
        .output()
        .expect("ace config get school");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), ".");
}

#[test]
fn config_get_trust_default() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let output = env
        .ace()
        .args(["config", "get", "trust"])
        .output()
        .expect("ace config get trust");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "default");
}

#[test]
fn config_get_env_key() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file("ace.toml", "school = \".\"\n\n[env]\nFOO = \"bar\"\n");

    let output = env
        .ace()
        .args(["config", "get", "env.FOO"])
        .output()
        .expect("ace config get env.FOO");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "bar");
}

#[test]
fn config_get_unknown_key() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "get", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown config key"));
}

// -- config set --

#[test]
fn config_set_backend_to_project() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "set", "backend", "codex"])
        .assert()
        .success();

    env.assert_contains("ace.toml", "backend = \"codex\"");
}

#[test]
fn config_set_trust_defaults_to_local() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "set", "trust", "auto"])
        .assert()
        .success();

    // Trust is personal-only → defaults to local scope
    env.assert_contains("ace.local.toml", "trust = \"auto\"");
}

#[test]
fn config_set_with_explicit_user_scope() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["--user", "config", "set", "backend", "codex"])
        .assert()
        .success();

    // Should be in user config, not project
    env.assert_contains("config/ace/ace.toml", "backend = \"codex\"");
    env.assert_not_contains("ace.toml", "codex");
}

#[test]
fn config_set_with_global_alias() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["--global", "config", "set", "trust", "yolo"])
        .assert()
        .success();

    env.assert_contains("config/ace/ace.toml", "trust = \"yolo\"");
}

#[test]
fn config_set_env_dot_path() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "set", "env.MY_KEY", "my_value"])
        .assert()
        .success();

    env.assert_contains("ace.toml", "MY_KEY = \"my_value\"");
}

#[test]
fn config_set_backend_field_model_for_builtin() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args([
            "config",
            "set",
            "backends.claude.model",
            "provider/model@beta",
        ])
        .assert()
        .success();

    env.assert_contains("ace.toml", "[backends.claude]");
    env.assert_contains("ace.toml", "model = \"provider/model@beta\"");
}

#[test]
fn config_set_backend_field_effort_preserves_cross_layer_custom_backend() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file(
        "config/ace/ace.toml",
        concat!(
            "[backends.bailer]\n",
            "kind = \"claude\"\n",
            "env = { API_TOKEN = \"secret\" }\n",
        ),
    );

    env.ace()
        .args(["config", "set", "backends.bailer.effort", "ultra"])
        .assert()
        .success();

    env.ace()
        .args(["config", "set", "backend", "bailer"])
        .assert()
        .success();

    env.assert_contains("config/ace/ace.toml", "kind = \"claude\"");
    env.assert_contains("config/ace/ace.toml", "API_TOKEN = \"secret\"");
    env.assert_contains("ace.toml", "effort = \"ultra\"");
    env.assert_contains("ace.toml", "backend = \"bailer\"");
}

#[test]
fn config_set_backend_field_supports_dotted_name_and_explicit_scope() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args([
            "--local",
            "config",
            "set",
            "backends.bedrock.claude.model",
            "opus",
        ])
        .assert()
        .success();

    env.assert_contains("ace.local.toml", "[backends.\"bedrock.claude\"]");
    env.assert_contains("ace.local.toml", "model = \"opus\"");
    env.assert_not_contains("ace.toml", "bedrock.claude");
}

#[test]
fn config_set_dotted_keys_preserves_inline_tables_and_siblings() {
    let env = TestEnv::new();
    env.write_file(
        "ace.toml",
        concat!(
            "env = { \"APP.MODE\" = \"old\", KEEP = \"yes\" } # environment\n",
            "[backends]\n",
            "\"bedrock.claude\" = { model = \"old\", future = \"keep\" } # provider\n",
        ),
    );

    env.ace()
        .args(["config", "set", "env.APP.MODE", "new"])
        .assert()
        .success();
    env.ace()
        .args(["config", "set", "backends.bedrock.claude.model", "opus"])
        .assert()
        .success();

    let written = env.read_file("ace.toml");
    assert!(written.contains("# environment"), "{written}");
    assert!(written.contains("# provider"), "{written}");
    let config: toml::Value = toml::from_str(&written).expect("valid edited TOML");
    assert_eq!(config["env"]["APP.MODE"].as_str(), Some("new"));
    assert_eq!(config["env"]["KEEP"].as_str(), Some("yes"));
    assert_eq!(
        config["backends"]["bedrock.claude"]["model"].as_str(),
        Some("opus")
    );
    assert_eq!(
        config["backends"]["bedrock.claude"]["future"].as_str(),
        Some("keep")
    );
}

#[cfg(unix)]
#[test]
fn config_set_preserves_symlink_and_target_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let env = TestEnv::new();
    env.write_file("personal.toml", "resume = true\n");
    env.symlink("personal.toml", "ace.local.toml");
    let target = env.path("personal.toml");
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o640))
        .expect("set config permissions");

    env.ace()
        .args(["config", "set", "resume", "false"])
        .assert()
        .success();

    env.assert_symlink("ace.local.toml", "personal.toml");
    env.assert_contains("personal.toml", "resume = false");
    let permissions = std::fs::metadata(target)
        .expect("stat target")
        .permissions();
    assert_eq!(permissions.mode() & 0o777, 0o640);
}

#[test]
fn config_set_backend_field_rejects_unsupported_path() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "set", "backends.claude.cmd", "wrapper"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown config key"));
}

#[test]
fn config_set_resume_to_local() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "set", "resume", "false"])
        .assert()
        .success();

    env.assert_contains("ace.local.toml", "resume = false");
}

#[test]
fn config_set_invalid_backend() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    let original = env.read_file("ace.toml");

    env.ace()
        .args(["config", "set", "backend", "invalid"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown backend"));

    assert_eq!(env.read_file("ace.toml"), original);
}

#[test]
fn config_set_invalid_trust() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    let original = "trust = \"auto\" # personal\nfuture = \"keep\"\n";
    env.write_file("ace.local.toml", original);

    env.ace()
        .args(["config", "set", "trust", "invalid"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("invalid trust value"));

    assert_eq!(env.read_file("ace.local.toml"), original);
}

// -- scope flag conflicts --

#[test]
fn scope_flags_conflict() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["--user", "--local", "config", "set", "trust", "auto"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "cannot combine multiple scope flags",
        ));
}

// -- user layer resolution --

#[test]
fn user_layer_provides_defaults() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    // Set backend at user level
    env.write_file("config/ace/ace.toml", "backend = \"codex\"\n");

    let output = env
        .ace()
        .args(["config", "get", "backend"])
        .output()
        .expect("ace config get backend");

    assert!(output.status.success());
    // Embedded setup writes ace.toml with no backend, so user layer should win.
    // But setup_embedded uses `ace setup .` which doesn't set backend.
    // Actually let me check — setup_embedded does git_init + setup_embedded_school + ace setup .
    // ace setup writes school=. but no backend. So user layer codex should be effective.
    // However, default backend fallback is claude. User layer codex should override that.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "codex");
}

#[test]
fn local_layer_overrides_user_layer_trust() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.write_file("config/ace/ace.toml", "trust = \"auto\"\n");
    env.write_file("ace.local.toml", "trust = \"yolo\"\n");

    let output = env
        .ace()
        .args(["config", "get", "trust"])
        .output()
        .expect("ace config get trust");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "yolo");
}

#[test]
fn user_layer_trust_used_when_no_local() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.write_file("config/ace/ace.toml", "trust = \"auto\"\n");

    let output = env
        .ace()
        .args(["config", "get", "trust"])
        .output()
        .expect("ace config get trust");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "auto");
}

// -- read-only inspection survives a stale backend selector --

#[test]
fn config_show_survives_unknown_backend() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file("ace.local.toml", "backend = \"no-such-backend\"\n");

    let output = env.ace().args(["config"]).output().expect("ace config");

    assert!(
        output.status.success(),
        "ace config show should succeed even with unknown backend"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("backend = \"no-such-backend\""),
        "should print the configured backend name verbatim, got: {stdout}"
    );
}

#[test]
fn config_get_backend_survives_unknown_backend() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file("ace.local.toml", "backend = \"no-such-backend\"\n");

    let output = env
        .ace()
        .args(["config", "get", "backend"])
        .output()
        .expect("ace config get backend");

    assert!(
        output.status.success(),
        "ace config get backend should succeed even with unknown backend"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "no-such-backend");
}

// -- yolo with scope --

// -- config explain --

#[test]
fn config_explain_shows_all_keys() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file("ace.local.toml", "trust = \"auto\"\n");

    let output = env
        .ace()
        .args(["config", "explain"])
        .output()
        .expect("ace config explain");

    assert!(output.status.success(), "ace config explain should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("school"), "should show school key");
    assert!(stdout.contains("backend"), "should show backend key");
    assert!(stdout.contains("trust = \"auto\""), "trust winner shown");
    assert!(stdout.contains("[local]"), "winner source label");
    assert!(stdout.contains("← winner"), "winner marker");
    assert!(stdout.contains("user:"), "per-layer breakdown");
    assert!(stdout.contains("project:"));
    assert!(stdout.contains("local:"));
    assert!(stdout.contains("override:"));
}

#[test]
fn config_explain_filters_to_one_key() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"phoenix\"\n");

    let output = env
        .ace()
        .args(["config", "explain", "backend"])
        .output()
        .expect("ace config explain backend");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("backend = \"flaude\""),
        "backend winner shown"
    );
    assert!(stdout.contains("[project]"), "project layer set backend");
    assert!(
        stdout.contains("school:"),
        "school row present in backend block"
    );
    assert!(!stdout.contains("trust"), "other keys filtered out");
    assert!(!stdout.contains("session_prompt"));
}

#[test]
fn config_explain_unknown_key() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "explain", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown config key"));
}

#[test]
fn config_explain_default_collapses_when_no_layer_set() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let output = env
        .ace()
        .args(["config", "explain", "trust"])
        .output()
        .expect("ace config explain trust");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("trust = \"default\""));
    assert!(stdout.contains("[default]"));
    // No breakdown rows when no layer contributes
    assert!(!stdout.contains("user:"), "should collapse: {stdout}");
}

// -- yolo with scope --

#[test]
fn yolo_with_user_scope() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace().args(["--user", "yolo"]).assert().success();

    env.assert_contains("config/ace/ace.toml", "trust = \"yolo\"");
}

#[test]
fn auto_with_user_scope() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file(
        "config/ace/ace.toml",
        "trust = \"default\" # personal trust\nfuture = \"keep\"\n",
    );

    env.ace().args(["--user", "auto"]).assert().success();

    env.assert_contains("config/ace/ace.toml", "trust = \"auto\"");
    env.assert_contains("config/ace/ace.toml", "# personal trust");
    env.assert_contains("config/ace/ace.toml", "future = \"keep\"");
}

// -- override-shaped CLI flags (--trust, --auto, --yolo, --session-prompt, --env) --

#[test]
fn override_trust_flag_wins() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let output = env
        .ace()
        .args(["--trust", "auto", "config", "explain", "trust"])
        .output()
        .expect("ace --trust auto config explain trust");

    assert!(output.status.success(), "should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("trust = \"auto\""),
        "winner shown: {stdout}"
    );
    assert!(
        stdout.contains("[override]"),
        "winner source label: {stdout}"
    );
}

#[test]
fn override_auto_shorthand() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let output = env
        .ace()
        .args(["--auto", "config", "explain", "trust"])
        .output()
        .expect("ace --auto config explain trust");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("trust = \"auto\""), "{stdout}");
    assert!(stdout.contains("[override]"), "{stdout}");
}

#[test]
fn override_yolo_shorthand() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let output = env
        .ace()
        .args(["--yolo", "config", "explain", "trust"])
        .output()
        .expect("ace --yolo config explain trust");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("trust = \"yolo\""), "{stdout}");
    assert!(stdout.contains("[override]"), "{stdout}");
}

#[test]
fn override_trust_bad_value() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["--trust", "nope", "config"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("trust"));
}

#[test]
fn override_trust_combine_errors() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["--auto", "--yolo", "config"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot combine"));
}

#[test]
fn override_session_prompt_flag_wins() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file(
        "ace.toml",
        "school = \"phoenix\"\nsession_prompt = \"from project\"\n",
    );

    let output = env
        .ace()
        .args([
            "--session-prompt",
            "live",
            "config",
            "explain",
            "session_prompt",
        ])
        .output()
        .expect("ace --session-prompt config explain");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("session_prompt = \"live\""), "{stdout}");
    assert!(stdout.contains("[override]"), "{stdout}");
}

#[test]
fn override_env_adds_entry() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let output = env
        .ace()
        .args(["--env", "BAR=baz", "config", "explain", "env.BAR"])
        .output()
        .expect("ace --env config explain");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("env.BAR = \"baz\""), "{stdout}");
    assert!(stdout.contains("[override]"), "{stdout}");
}

#[test]
fn override_env_overrides_existing() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file(
        "ace.toml",
        "school = \"phoenix\"\n\n[env]\nFOO = \"from project\"\n",
    );

    let output = env
        .ace()
        .args(["--env", "FOO=from-cli", "config", "explain", "env.FOO"])
        .output()
        .expect("ace --env config explain");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("env.FOO = \"from-cli\""), "{stdout}");
    assert!(stdout.contains("[override]"), "{stdout}");
}

#[test]
fn override_env_repeated() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let out_a = env
        .ace()
        .args(["--env", "A=1", "--env", "B=2", "config", "explain", "env.A"])
        .output()
        .expect("explain env.A");
    assert!(out_a.status.success());
    let stdout_a = String::from_utf8_lossy(&out_a.stdout);
    assert!(stdout_a.contains("env.A = \"1\""), "{stdout_a}");
    assert!(stdout_a.contains("[override]"), "{stdout_a}");

    let out_b = env
        .ace()
        .args(["--env", "A=1", "--env", "B=2", "config", "explain", "env.B"])
        .output()
        .expect("explain env.B");
    assert!(out_b.status.success());
    let stdout_b = String::from_utf8_lossy(&out_b.stdout);
    assert!(stdout_b.contains("env.B = \"2\""), "{stdout_b}");
    assert!(stdout_b.contains("[override]"), "{stdout_b}");
}

#[test]
fn override_env_bad_format() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["--env", "NOEQUALS", "config"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("KEY=VAL"));
}
