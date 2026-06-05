mod common;

use common::TestEnv;

// -- ace upgrade command exists --

#[test]
fn upgrade_help() {
    let env = TestEnv::new();
    env.ace().args(["upgrade", "--help"]).assert().success();
}

// -- skip_update config key --

#[test]
fn config_get_skip_update_default_false() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    let output = env
        .ace()
        .args(["config", "get", "skip_update"])
        .output()
        .expect("ace config get skip_update");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.trim(), "false");
}

#[test]
fn config_set_skip_update() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "set", "skip_update", "true"])
        .assert()
        .success();

    env.assert_contains("ace.toml", "skip_update = true");
}

#[test]
fn config_set_skip_update_local_scope() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["--local", "config", "set", "skip_update", "true"])
        .assert()
        .success();

    env.assert_contains("ace.local.toml", "skip_update = true");
}

#[test]
fn config_set_skip_update_invalid_value() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .args(["config", "set", "skip_update", "invalid"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("expected true or false"));
}

// -- skip via env var --

#[test]
fn upgrade_skipped_with_env_var() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");

    env.ace()
        .env("ACE_SKIP_UPDATE", "1")
        .args(["upgrade"])
        .assert()
        .success();
}

// -- skip_update in effective config --

#[test]
fn config_shows_skip_update_when_set() {
    let env = TestEnv::new();
    env.setup_embedded("phoenix");
    env.write_file("ace.toml", "school = \".\"\nskip_update = true\n");

    let output = env.ace().args(["config"]).output().expect("ace config");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("skip_update = true"),
        "effective config should show skip_update"
    );
}
