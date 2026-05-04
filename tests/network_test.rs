mod common;

use common::TestEnv;

#[test]
#[ignore] // requires network — run with `cargo test -- --ignored`
fn setup_remote_school() {
    let env = TestEnv::new();
    env.git_init();

    env.ace()
        .args(["setup", "ace-rs/school"])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .success();

    // ace.toml written with remote specifier.
    env.assert_exists("ace.toml");
    env.assert_contains("ace.toml", "ace-rs/school");

    // Skills symlinked from school clone.
    env.assert_exists(".claude/skills");

    // CLAUDE.md generated.
    env.assert_exists("CLAUDE.md");
}

#[test]
#[ignore] // requires network — run with `cargo test -- --ignored`
fn setup_remote_then_rerun() {
    let env = TestEnv::new();
    env.git_init();

    // First setup.
    env.ace()
        .args(["setup", "ace-rs/school"])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .success();

    env.assert_exists("ace.toml");
    env.assert_exists(".claude/skills");

    // Remove ace.toml to allow re-setup.
    std::fs::remove_file(env.path("ace.toml")).expect("remove ace.toml");

    // Re-setup — should succeed using cached clone.
    env.ace()
        .args(["setup", "ace-rs/school"])
        .timeout(std::time::Duration::from_secs(10))
        .assert()
        .success();

    // ace.toml restored.
    env.assert_exists("ace.toml");
    env.assert_contains("ace.toml", "ace-rs/school");

    // Skills symlink still valid.
    env.assert_exists(".claude/skills");
}
