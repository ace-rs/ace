mod common;

use common::TestEnv;

#[test]
fn fmt_ace_toml() {
    let env = TestEnv::new();

    // Messy ace.toml with extra whitespace.
    env.write_file("ace.toml", "school   =   \".\"\n\n\n\n");

    env.ace().args(["fmt"]).assert().success();

    // Should be normalized — load + save round-trip cleans it up.
    let content = env.read_file("ace.toml");
    assert!(content.contains("school"), "should still have school field");
    // Extra blank lines should be gone after pretty-print.
    assert!(
        !content.contains("\n\n\n"),
        "extra blank lines should be removed"
    );
}

#[test]
fn fmt_school_toml() {
    let env = TestEnv::new();

    // school.toml with empty optional fields that should be stripped.
    env.write_file("school.toml", "name = \"test\"\nsession_prompt = \"\"\n");

    env.ace().args(["fmt"]).assert().success();

    let content = env.read_file("school.toml");
    assert!(content.contains("test"), "name should be preserved");
    // Empty session_prompt should be stripped by skip_serializing_if.
    assert!(
        !content.contains("session_prompt"),
        "empty session_prompt should be stripped"
    );
}

#[test]
fn fmt_both_files() {
    let env = TestEnv::new();

    env.write_file("ace.toml", "school = \".\"\n");
    env.write_file("school.toml", "name = \"test\"\n");

    env.ace().args(["fmt"]).assert().success();

    // Both files should still be valid.
    env.assert_exists("ace.toml");
    env.assert_exists("school.toml");
}

#[test]
fn fmt_no_files() {
    let env = TestEnv::new();

    // No ace.toml or school.toml — should exit successfully with warning.
    env.ace()
        .args(["fmt"])
        .assert()
        .success()
        .stderr(predicates::str::contains("no ace.toml or school.toml"));
}
