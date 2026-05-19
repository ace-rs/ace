mod common;

use common::TestEnv;

#[test]
fn school_init_creates_files() {
    let env = TestEnv::new();
    env.git_init();

    env.ace()
        .args(["school", "init", "--name", "test-school"])
        .assert()
        .success();

    env.assert_exists("school.toml");
    env.assert_contains("school.toml", "test-school");
    env.assert_exists("CLAUDE.md");
    env.assert_exists("README.md");
    env.assert_exists("skills/ace-school/SKILL.md");
}

#[test]
fn school_init_not_in_git_repo() {
    let env = TestEnv::new();
    // No git init.

    env.ace()
        .args(["school", "init", "--name", "test-school"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git"));
}

#[test]
fn school_init_already_exists() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"old-school\"\n");

    env.ace()
        .args(["school", "init", "--name", "new-school"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("already exists"));
}

#[test]
fn school_init_force_updates_name() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"old-school\"\n");

    env.ace()
        .args(["school", "init", "--force", "--name", "new-school"])
        .assert()
        .success();

    env.assert_contains("school.toml", "new-school");
    env.assert_not_contains("school.toml", "old-school");
}

#[test]
fn school_init_writes_dogfood_ace_toml() {
    let env = TestEnv::new();
    env.git_init();

    env.ace()
        .args(["school", "init", "--name", "test-school"])
        .assert()
        .success();

    env.assert_exists("ace.toml");
    env.assert_contains("ace.toml", "school = \".\"");
}

#[test]
fn school_init_preserves_existing_ace_toml() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("ace.toml", "school = \"jedi/temple\"\n");

    env.ace()
        .args(["school", "init", "--name", "test-school"])
        .assert()
        .success();

    // Existing ace.toml is not overwritten.
    env.assert_contains("ace.toml", "jedi/temple");
}

#[test]
fn school_init_preserves_existing_files() {
    let env = TestEnv::new();
    env.git_init();

    // Pre-existing CLAUDE.md should not be overwritten.
    env.write_file("CLAUDE.md", "# My Custom Instructions\n");

    env.ace()
        .args(["school", "init", "--name", "test-school"])
        .assert()
        .success();

    // school.toml created.
    env.assert_exists("school.toml");
    env.assert_contains("school.toml", "test-school");

    // CLAUDE.md preserved — still has custom content.
    env.assert_contains("CLAUDE.md", "My Custom Instructions");
}
