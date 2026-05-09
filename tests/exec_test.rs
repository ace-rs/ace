mod common;

use common::TestEnv;

#[test]
fn exec_records_session() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    env.ace().assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1, "should record one exec call");
    assert_eq!(records[0].trust, "default", "default trust level");
    assert!(!records[0].session_prompt.is_empty(), "session prompt should be non-empty");
}

#[test]
fn one_shot_routes_to_exec_one_shot_with_inline_prompt() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    env.ace().args(["-p", "what is rust"]).assert().success();

    let session = env.read_flaude_exec_records();
    assert!(
        session.is_empty(),
        "ace -p should not trigger an interactive session record",
    );

    let one_shot = env.read_flaude_one_shot_records();
    assert_eq!(one_shot.len(), 1, "should record one exec_one_shot call");
    assert_eq!(one_shot[0].prompt_kind.as_deref(), Some("inline"));
    assert_eq!(one_shot[0].prompt_text.as_deref(), Some("what is rust"));
}

#[test]
fn bare_ace_routes_to_exec_session() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    env.ace().assert().success();

    assert_eq!(env.read_flaude_exec_records().len(), 1);
    assert!(
        env.read_flaude_one_shot_records().is_empty(),
        "bare ace must not trigger one-shot path",
    );
}

#[test]
fn exec_yolo_records_trust() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file("ace.local.toml", "trust = \"yolo\"\n");

    let output = env.ace().output().expect("ace run");

    assert!(output.status.success(), "ace should succeed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("yolo mode"), "should warn about yolo mode");

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1, "should record one exec call");
    assert_eq!(records[0].trust, "yolo", "trust should be yolo");
}

#[test]
fn exec_auto_records_trust() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file("ace.local.toml", "trust = \"auto\"\n");

    let output = env.ace().output().expect("ace run");

    assert!(output.status.success(), "ace should succeed");

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1, "should record one exec call");
    assert_eq!(records[0].trust, "auto", "trust should be auto");
}

#[test]
fn exec_backcompat_yolo_true() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file("ace.local.toml", "yolo = true\n");

    let output = env.ace().output().expect("ace run");

    assert!(output.status.success(), "ace should succeed");

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1, "should record one exec call");
    assert_eq!(records[0].trust, "yolo", "yolo=true backcompat should record trust=yolo");
}

#[test]
fn exec_custom_backend_records_cmd() {
    // A custom backend with kind=flaude and an explicit cmd should land
    // intact in the recorded SessionOpts.cmd — proves Backend.cmd[0] flows
    // through Backend::exec_session into the per-backend exec.
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file(
        "ace.local.toml",
        "[[backends]]\nname = \"myflaude\"\nkind = \"flaude\"\ncmd = [\"my-binary\", \"--flag\"]\n",
    );

    env.ace().args(["--backend", "myflaude"]).assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].cmd, vec!["my-binary", "--flag"]);
}

#[test]
fn exec_builtin_backend_records_default_cmd() {
    // Built-in flaude (no [[backends]] override) should still record cmd —
    // defaulted to [kind.name()].
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    env.ace().assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].cmd, vec!["flaude"]);
}

#[test]
fn exec_backend_flag_overrides_configured_backend() {
    // Declare a second flaude-kind backend and select it via --backend.
    // The recorded cmd proves the override took effect.
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file(
        "ace.local.toml",
        "[[backends]]\nname = \"alt\"\nkind = \"flaude\"\ncmd = [\"alt-binary\"]\n",
    );

    env.ace().args(["--backend", "alt"]).assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].cmd, vec!["alt-binary"], "should use overridden backend cmd");
}

// -- resume flag --

#[test]
fn exec_session_default_resumes() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    env.ace().assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1);
    assert!(records[0].resume, "bare ace should default to resume=true");
}

#[test]
fn exec_new_does_not_resume() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    env.ace().args(["new"]).assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1);
    assert!(!records[0].resume, "ace new should set resume=false");
    assert!(!records[0].session_prompt.is_empty(), "new session should include prompt");
}

// -- one-shot has no trust/resume --

#[test]
fn one_shot_omits_trust_and_resume() {
    // One-shot uses OneShotRequest which has no trust/resume fields.
    // Verify the recorded JSON has no trust or resume keys.
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file("ace.local.toml", "trust = \"yolo\"\n");

    env.ace().args(["-p", "hello"]).assert().success();

    let records = env.read_flaude_one_shot_records();
    assert_eq!(records.len(), 1);
    // FlaudeRecord defaults: trust="" and resume=false when keys are absent.
    assert_eq!(records[0].trust, "", "one-shot should not carry trust");
    assert!(!records[0].resume, "one-shot should not carry resume");
    assert_eq!(records[0].session_prompt, "", "one-shot should not carry session_prompt");
}

// -- env merging --

#[test]
fn custom_backend_env_reaches_exec() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file(
        "ace.local.toml",
        "[[backends]]\nname = \"myflaude\"\nkind = \"flaude\"\n\n[backends.env]\nMY_VAR = \"hello\"\n",
    );

    env.ace().args(["--backend", "myflaude"]).assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].env.get("MY_VAR").map(String::as_str),
        Some("hello"),
        "per-backend env should reach exec, got: {:?}",
        records[0].env,
    );
}

#[test]
fn project_env_merges_into_exec() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");
    env.write_file("ace.toml", "school = \".\"\nbackend = \"flaude\"\n\n[env]\nFOO = \"bar\"\n");

    env.ace().assert().success();

    let records = env.read_flaude_exec_records();
    assert_eq!(records.len(), 1);
    assert_eq!(
        records[0].env.get("FOO").map(String::as_str),
        Some("bar"),
        "project-level env should reach exec, got: {:?}",
        records[0].env,
    );
}

// -- one-shot exit code and output --

#[test]
fn one_shot_exit_code_propagates() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    let output = env.ace()
        .env("FLAUDE_ONE_SHOT_EXIT_CODE", "42")
        .args(["-p", "test"])
        .output()
        .expect("ace run");

    assert!(!output.status.success(), "should fail");
    assert_eq!(output.status.code(), Some(42), "exit code should propagate");
}

#[test]
fn one_shot_stdout_passthrough() {
    let env = TestEnv::new();
    env.setup_flaude_school("name = \"test-school\"\n");

    let output = env.ace()
        .env("FLAUDE_ONE_SHOT_STDOUT", "hello from agent")
        .args(["-p", "test"])
        .output()
        .expect("ace run");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("hello from agent"), "stdout should pass through, got: {stdout}");
}
