mod common;

use common::TestEnv;

#[test]
fn import_no_school_context() {
    let env = TestEnv::new();
    env.git_init();

    // No school.toml, no ace.toml — should fail with "no config" error.
    env.ace()
        .args(["import", "owner/repo", "--skill", "my-skill"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no config found"));
}

#[test]
fn import_clone_failure_invalid_source() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    // Source that cannot be cloned — nonexistent GitHub repo.
    env.ace()
        .args(["import", "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx", "--skill", "my-skill"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn import_requires_source_argument() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");

    // Missing required <source> argument.
    env.ace()
        .args(["import"])
        .assert()
        .failure();
}

#[test]
fn import_from_local_school_context() {
    let env = TestEnv::new();
    env.git_init();

    // School repo context (school.toml present) but invalid remote source.
    env.write_file("school.toml", "name = \"my-school\"\n");
    env.mkdir("skills");

    // The source is invalid, so clone fails — but this verifies that import
    // correctly detects the school context via school.toml.
    env.ace()
        .args(["import", "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn import_without_skill_flag_clone_failure() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    // No --skill flag — auto-select or prompt would happen after clone.
    // Clone fails first, so we verify the error path without --skill.
    env.ace()
        .args(["import", "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn import_no_git_repo_with_school_toml() {
    let env = TestEnv::new();
    // No git init — but school.toml exists.
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    // Import should still work to find school context (school.toml check
    // doesn't require git), but clone will fail on the remote source.
    env.ace()
        .args(["import", "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx", "--skill", "x"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn import_skill_flag_requires_value() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");

    // --skill without a value should be a clap argument error.
    env.ace()
        .args(["import", "owner/repo", "--skill"])
        .assert()
        .failure();
}

#[test]
fn import_with_existing_imports_clone_failure() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file(
        "school.toml",
        r#"name = "test-school"

[[imports]]
skill = "existing-skill"
source = "some-owner/some-repo"
"#,
    );
    env.mkdir("skills/existing-skill");
    env.write_file("skills/existing-skill/SKILL.md", "# Existing\n");

    // Importing a new skill from an invalid source fails at clone.
    // Verifies that having existing imports doesn't break the import flow.
    env.ace()
        .args(["import", "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx", "--skill", "new-skill"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn import_all_adds_wildcard_entry() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    // --all writes a wildcard import entry without cloning (no network needed).
    env.ace()
        .args(["import", "company/school", "--all"])
        .assert()
        .success()
        .stderr(predicates::str::contains("Added import: * from company/school"));

    let toml = env.read_file("school.toml");
    assert!(toml.contains("skill = \"*\""), "should have wildcard skill entry");
    assert!(toml.contains("source = \"company/school\""), "should have source");
}

#[test]
fn import_glob_pattern_adds_entry() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args(["import", "company/school", "--skill", "*-coding"])
        .assert()
        .success()
        .stderr(predicates::str::contains("Added import: *-coding from company/school"));

    let toml = env.read_file("school.toml");
    assert!(toml.contains("skill = \"*-coding\""), "should have glob pattern");
}

#[test]
fn import_all_duplicate_warns() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file(
        "school.toml",
        r#"name = "test-school"

[[imports]]
skill = "*"
source = "company/school"
"#,
    );
    env.mkdir("skills");

    env.ace()
        .args(["import", "company/school", "--all"])
        .assert()
        .success()
        .stderr(predicates::str::contains("import already exists"));
}

// -- tier-inclusion flags (PROD9-75) --

#[test]
fn import_include_experimental_without_all_errors() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args(["import", "owner/repo", "--include-experimental"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--all"));
}

#[test]
fn import_include_system_without_all_errors() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args(["import", "owner/repo", "--include-system"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--all"));
}

#[test]
fn import_include_with_explicit_skill_errors() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args(["import", "owner/repo", "--skill", "foo", "--include-experimental"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--all"));
}

#[test]
fn import_all_include_experimental_persists_flag() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args(["import", "company/school", "--all", "--include-experimental"])
        .assert()
        .success();

    let toml = env.read_file("school.toml");
    assert!(toml.contains("include_experimental = true"), "missing flag in {toml}");
    assert!(!toml.contains("include_system"), "include_system should not be written: {toml}");
}

#[test]
fn import_all_include_both_flags_persists_both() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args(["import", "company/school", "--all", "--include-experimental", "--include-system"])
        .assert()
        .success();

    let toml = env.read_file("school.toml");
    assert!(toml.contains("include_experimental = true"), "missing experimental flag: {toml}");
    assert!(toml.contains("include_system = true"), "missing system flag: {toml}");
}

// -- end-to-end import with real git (PROD9-75) --

#[test]
fn import_explicit_skill_resolves_from_experimental_tier() {
    // Reproduces the original bug: shell lives in skills/.experimental/ only.
    // Before PROD9-75, ACE skipped all hidden dirs and reported "no skills found".
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("dot/skills", &[
        "skills/.experimental/shell",
        "skills/.curated/react",
    ]);

    env.ace()
        .args(["import", "dot/skills", "--skill", "shell"])
        .assert()
        .success();

    env.assert_exists("skills/shell/SKILL.md");
    env.assert_contains("school.toml", "skill = \"shell\"");
    env.assert_contains("school.toml", "source = \"dot/skills\"");
}

#[test]
fn import_all_defaults_to_curated_tier_only() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("dot/skills", &[
        "skills/.curated/react",
        "skills/.experimental/shell",
        "skills/.system/skill-creator",
    ]);

    // --all without --include-* flags should record a wildcard entry only.
    env.ace()
        .args(["import", "dot/skills", "--all"])
        .assert()
        .success();

    // The actual expansion happens on school update.
    env.ace()
        .args(["school", "update"])
        .assert()
        .success();

    env.assert_exists("skills/react/SKILL.md");
    env.assert_not_exists("skills/shell/SKILL.md");
    env.assert_not_exists("skills/skill-creator/SKILL.md");
}

#[test]
fn import_all_with_include_experimental_pulls_that_tier() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("dot/skills", &[
        "skills/.curated/react",
        "skills/.experimental/shell",
        "skills/.system/skill-creator",
    ]);

    env.ace()
        .args(["import", "dot/skills", "--all", "--include-experimental"])
        .assert()
        .success();

    env.ace()
        .args(["school", "update"])
        .assert()
        .success();

    env.assert_exists("skills/react/SKILL.md");
    env.assert_exists("skills/shell/SKILL.md");
    env.assert_not_exists("skills/skill-creator/SKILL.md");
}

// PROD9-76: imports should persist the source clone under the cache dir so that
// subsequent imports from the same source fetch instead of re-cloning.
#[test]
fn import_populates_persistent_source_cache() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("cached/source", &[
        "skills/foo",
        "skills/bar",
    ]);

    env.ace()
        .args(["import", "cached/source", "--skill", "foo"])
        .assert()
        .success();

    let cache_path = env.path("cache/ace/imports/cached/source");
    assert!(
        cache_path.exists(),
        "import should populate persistent cache at {cache_path:?}",
    );
    assert!(
        cache_path.join(".git").exists(),
        "cache entry should be a git repo",
    );
}

// PROD9-187: when a `*` import overlaps with an explicit decl for the same
// skill name, pull-imports must dedup so disk doesn't get clobbered twice and
// must surface the collision so the user can de-conflict their school.toml.
#[test]
fn pull_imports_dedups_overlapping_sources_and_warns() {
    let env = TestEnv::new();
    env.git_init();

    env.setup_tiered_origin("anthropics/skills", &[
        "skills/.system/skill-creator",
    ]);
    env.setup_tiered_origin("ace-rs/school", &[
        "skills/.system/skill-creator",
        "skills/.curated/other-skill",
    ]);

    // Explicit decl listed first; `*` import second. With --include-system
    // both sources expose skill-creator, so without dedup the school's
    // skill-creator/ dir gets clobbered twice.
    env.write_file(
        "school.toml",
        r#"name = "test-school"

[[imports]]
skill = "skill-creator"
source = "anthropics/skills"
include_system = true

[[imports]]
skill = "*"
source = "ace-rs/school"
include_system = true
"#,
    );
    env.mkdir("skills");

    let assert = env.ace().args(["school", "update"]).assert().success();
    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    // One summary line per skill — no duplicate ~skill-creator.
    let creator_lines = combined.matches("skill-creator").count();
    assert!(
        creator_lines >= 1,
        "expected skill-creator to appear in output: {combined}"
    );
    let summary_dupes = combined.matches("~skill-creator").count()
        + combined.matches("+skill-creator").count();
    assert_eq!(
        summary_dupes, 1,
        "skill-creator appears {summary_dupes} times in summary; expected exactly 1: {combined}"
    );

    // Shadow warning surfaces both source labels so user can de-conflict.
    assert!(
        combined.contains("anthropics/skills") && combined.contains("ace-rs/school"),
        "expected shadow warning naming both sources: {combined}"
    );

    // Other-skill from the `*` import still lands.
    env.assert_exists("skills/skill-creator/SKILL.md");
    env.assert_exists("skills/other-skill/SKILL.md");
}

#[test]
fn import_reuses_source_cache_on_second_call() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("cached/source", &[
        "skills/foo",
        "skills/bar",
    ]);

    env.ace()
        .args(["import", "cached/source", "--skill", "foo"])
        .assert()
        .success();

    // Drop a sentinel inside the cache. A re-clone would wipe it; a fetch preserves it.
    let cache_path = env.path("cache/ace/imports/cached/source");
    let sentinel = cache_path.join(".ace-test-sentinel");
    std::fs::write(&sentinel, "preserve me").expect("write sentinel");

    env.ace()
        .args(["import", "cached/source", "--skill", "bar"])
        .assert()
        .success();

    assert!(
        sentinel.exists(),
        "second import should reuse cache via fetch, not re-clone; sentinel was wiped",
    );
}
