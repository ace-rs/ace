mod common;

use common::TestEnv;

#[test]
fn yolo_writes_trust_to_local_toml() {
    let env = TestEnv::new();
    env.setup_embedded("test-school");

    env.ace().arg("yolo").assert().success();

    env.assert_contains("ace.local.toml", "trust = \"yolo\"");
}

#[test]
fn auto_writes_trust_to_local_toml() {
    let env = TestEnv::new();
    env.setup_embedded("test-school");

    env.ace().arg("auto").assert().success();

    env.assert_contains("ace.local.toml", "trust = \"auto\"");
}

#[test]
fn yolo_preserves_existing_local_fields() {
    let env = TestEnv::new();
    env.setup_embedded("test-school");
    env.write_file("ace.local.toml", "backend = \"codex\"\n");

    env.ace().arg("yolo").assert().success();

    env.assert_contains("ace.local.toml", "trust = \"yolo\"");
    env.assert_contains("ace.local.toml", "backend = \"codex\"");
}

#[test]
fn yolo_clears_deprecated_yolo_field() {
    let env = TestEnv::new();
    env.setup_embedded("test-school");
    env.write_file("ace.local.toml", "yolo = true\n");

    env.ace().arg("yolo").assert().success();

    env.assert_contains("ace.local.toml", "trust = \"yolo\"");
    let content = std::fs::read_to_string(env.path("ace.local.toml")).expect("read local toml");
    assert!(
        !content.contains("yolo = true"),
        "deprecated yolo field should be cleared"
    );
}
