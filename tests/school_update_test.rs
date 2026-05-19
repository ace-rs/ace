mod common;

use common::TestEnv;

#[test]
fn school_update_no_imports() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");

    // No [[imports]] entries — should warn "no imports to pull".
    env.ace()
        .args(["school", "update"])
        .assert()
        .success()
        .stderr(predicates::str::contains("no imports to pull"));
}

#[test]
fn school_update_no_school_context() {
    let env = TestEnv::new();
    env.git_init();

    // No school.toml, no ace.toml — should fail with "no config" error.
    env.ace()
        .args(["school", "update"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no config found"));
}

#[test]
fn school_update_clone_failure() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
skill = "some-skill"
source = "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx"
"#,
    );
    env.mkdir("skills/some-skill");
    env.write_file("skills/some-skill/SKILL.md", "# Some Skill\n");

    // Source repo doesn't exist — clone should fail.
    env.ace()
        .args(["school", "update"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn school_update_empty_imports_array() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\nimports = []\n");

    // Explicit empty imports array — same as no imports.
    env.ace()
        .args(["school", "update"])
        .assert()
        .success()
        .stderr(predicates::str::contains("no imports to pull"));
}

#[test]
fn school_update_multiple_imports_same_source_clone_failure() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
skill = "skill-a"
source = "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx"

[[imports]]
skill = "skill-b"
source = "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx"
"#,
    );
    env.mkdir("skills/skill-a");
    env.write_file("skills/skill-a/SKILL.md", "# Skill A\n");
    env.mkdir("skills/skill-b");
    env.write_file("skills/skill-b/SKILL.md", "# Skill B\n");

    // Multiple imports from the same source — grouped into one clone attempt.
    // Clone fails, so we get the error.
    env.ace()
        .args(["school", "update"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn school_update_without_git_repo() {
    let env = TestEnv::new();
    // No git init, but the dogfood pair (school.toml + ace.toml) exists.
    env.write_dogfood_school("name = \"test-school\"\n");

    // School context is resolved via ace.toml's `school = "."` specifier (no
    // git required for require_school). No imports → warns and succeeds.
    env.ace()
        .args(["school", "update"])
        .assert()
        .success()
        .stderr(predicates::str::contains("no imports to pull"));
}

#[test]
fn school_update_preserves_non_imported_skills() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
skill = "imported-skill"
source = "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx"
"#,
    );

    // Non-imported skill should not be affected by update.
    env.mkdir("skills/local-skill");
    env.write_file("skills/local-skill/SKILL.md", "# Local Skill\n");
    env.mkdir("skills/imported-skill");
    env.write_file("skills/imported-skill/SKILL.md", "# Imported\n");

    // Update fails due to clone, but local skill should still be on disk.
    env.ace()
        .args(["school", "update"])
        .assert()
        .failure();

    // Local skill untouched after failed update.
    env.assert_exists("skills/local-skill/SKILL.md");
    env.assert_contains("skills/local-skill/SKILL.md", "Local Skill");
}
