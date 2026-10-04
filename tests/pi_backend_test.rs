mod common;

use common::TestEnv;
use predicates::prelude::*;

#[test]
fn pi_selection_allows_configuration_inspection_without_installation() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "backend = \"pi\"\n");
    env.ace()
        .args(["--pi", "config", "get", "backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pi"));
}

#[test]
fn pi_link_emits_nested_skills_and_preserves_shared_agent_skills() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    env.write_file("ace.toml", "school = \".\"\nbackend = \"pi\"\n");
    env.write_file("skills/rust/review/SKILL.md", "# Rust review\n");
    env.write_file(".agents/skills/personal/SKILL.md", "# Personal skill\n");

    env.ace().arg("link").assert().success();

    env.assert_symlink(".pi/skills/rust/review", "skills/rust/review");
    assert_eq!(
        env.read_file(".agents/skills/personal/SKILL.md"),
        "# Personal skill\n"
    );
}
