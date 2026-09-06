mod common;

use common::TestEnv;

#[test]
fn config_diagnostics_include_school_backend_declarations() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "school = \".\"\n");
    env.write_file(
        "school.toml",
        "name = \"example\"\nbackend = \"codex\"\n\
         [backends.codex]\neffort = \"high\"\ntypo = \"private-school-value\"\n",
    );

    let output = env
        .ace()
        .args(["config", "get", "backends.codex.effort"])
        .output()
        .expect("inspect school backend configuration");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "high");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("school.toml"),
        "missing school file: {stderr}"
    );
    assert!(
        stderr.contains("backends.codex.typo"),
        "missing school field: {stderr}"
    );
    assert!(!stderr.contains("private-school-value"));
}

#[test]
fn config_diagnostics_identify_ignored_fields_without_exposing_values() {
    let env = TestEnv::new();
    env.write_file(
        "ace.toml",
        "backend = \"codex\"\neffort = \"private-top-value\"\n\
         [backends.\"work.codex\"]\nkind = \"codex\"\n\
         typo = \"private-backend-value\"\n",
    );
    env.write_file("ace.local.toml", "[connect]\nenabled = true\n");

    let output = env
        .ace()
        .args(["config", "get", "backend"])
        .output()
        .expect("inspect config containing unsupported fields");

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "codex");
    let stderr = String::from_utf8_lossy(&output.stderr);
    for field in ["effort", "work.codex", "typo", "connect.enabled"] {
        assert!(
            stderr.contains(field),
            "missing diagnostic for {field}: {stderr}"
        );
    }
    for filename in ["ace.toml", "ace.local.toml"] {
        assert!(stderr.contains(filename), "missing source file: {stderr}");
    }
    assert!(!stderr.contains("private-top-value"));
    assert!(!stderr.contains("private-backend-value"));
}

#[test]
fn config_edit_preserves_and_reports_unknown_fields() {
    let env = TestEnv::new();
    let preserved = "# future integration\nfuture_setting = \"private-preserved-value\"\n";
    env.write_file("ace.toml", preserved);

    let output = env
        .ace()
        .args(["config", "set", "session_prompt", "hello"])
        .output()
        .expect("edit config containing an unknown field");

    assert!(output.status.success());
    let content = std::fs::read_to_string(env.path("ace.toml")).expect("read edited config");
    assert!(content.contains(preserved));
    assert!(content.contains("session_prompt = \"hello\""));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("future_setting"),
        "missing diagnostic: {stderr}"
    );
    assert!(!stderr.contains("private-preserved-value"));
}
