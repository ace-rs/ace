mod common;

use std::path::Path;

use common::{RemoteSchool, TestEnv};
use predicates::prelude::*;

// -- Helpers --

/// Push a new commit to origin via a temp working copy.
fn push_to_origin(env: &TestEnv, school: &RemoteSchool, file: &str, content: &str) {
    let work = env.path("_push_work");
    let origin_str = school.origin.to_str().expect("origin path");
    let work_str = work.to_str().expect("work path");

    env.git_in(env.root(), &["clone", "--quiet", origin_str, work_str]);
    std::fs::write(work.join(file), content).expect("write file");
    env.git_in(&work, &["add", "-A"]);
    env.git_in(
        &work,
        &[
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            "update from origin",
        ],
    );
    env.git_in(&work, &["push"]);
    std::fs::remove_dir_all(&work).expect("cleanup work dir");
}

/// Remove FETCH_HEAD so Update considers the cache stale.
fn make_stale(school: &RemoteSchool) {
    let _ = std::fs::remove_file(school.cache.join(".git/FETCH_HEAD"));
}

/// Read the current branch name of a repo.
fn current_branch(env: &TestEnv, dir: &Path) -> String {
    env.git_in(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_string()
}

// -- Smoke tests --

#[test]
fn setup_remote_school_smoke() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Cache is a valid git clone.
    assert!(school.cache.join(".git").exists(), "cache should have .git");
    assert!(
        school.cache.join("school.toml").exists(),
        "cache should have school.toml"
    );
    assert!(
        school.cache.join("skills/maverick/SKILL.md").exists(),
        "cache should have skills"
    );

    // Origin is a bare repo.
    assert!(
        school.origin.join("HEAD").exists(),
        "origin should be bare repo"
    );

    // Index entry exists.
    env.assert_contains("data/ace/index.toml", "test/school");

    // Project dir has ace.toml and is a git repo.
    env.assert_contains("ace.toml", "school = \"test/school\"");
    env.assert_contains("ace.toml", "backend = \"flaude\"");

    // Cache clone can fetch from origin.
    env.git_in(&school.cache, &["fetch", "origin", "main"]);
}

#[test]
fn remote_school_ace_flaude_runs() {
    let env = TestEnv::new();
    let _school = env.setup_remote_school("test/school");

    env.ace().assert().success();

    // Verify ace linked skills into project.
    env.assert_exists(".claude/skills");
}

// -- Update behavior tests --

#[test]
fn dirty_cache_warns() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Dirty the cache working tree.
    std::fs::write(school.cache.join("dirty.txt"), "uncommitted").expect("dirty file");

    env.ace()
        .assert()
        .success()
        .stderr(predicates::str::contains("local changes").or(predicates::str::contains("dirty")));
}

#[test]
fn dirty_non_main_warns_with_branch() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Switch to a feature branch and dirty it.
    env.git_in(&school.cache, &["checkout", "-b", "feature-x"]);
    std::fs::write(school.cache.join("dirty.txt"), "uncommitted").expect("dirty file");

    env.ace()
        .assert()
        .success()
        .stderr(predicates::str::contains("feature-x"));
}

#[test]
fn clean_non_main_switches_to_main() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Switch to a clean feature branch.
    env.git_in(&school.cache, &["checkout", "-b", "feature-y"]);

    env.ace().assert().success();

    // Verify cache is back on main.
    assert_eq!(
        current_branch(&env, &school.cache),
        "main",
        "cache should be back on main"
    );
}

#[test]
fn ahead_of_origin_warns() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Add a local-only commit to cache.
    std::fs::write(school.cache.join("local.txt"), "local only").expect("write local");
    env.git_in(&school.cache, &["add", "-A"]);
    env.git_in(
        &school.cache,
        &[
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            "local commit",
        ],
    );
    make_stale(&school);

    env.ace()
        .assert()
        .success()
        .stderr(predicates::str::contains("local commits"));
}

#[test]
fn diverged_warns() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Push a new commit to origin.
    push_to_origin(&env, &school, "remote.txt", "from origin");

    // Add a different local commit to cache (diverges from origin).
    std::fs::write(school.cache.join("local.txt"), "local only").expect("write local");
    env.git_in(&school.cache, &["add", "-A"]);
    env.git_in(
        &school.cache,
        &[
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            "local divergent commit",
        ],
    );
    make_stale(&school);

    env.ace()
        .assert()
        .success()
        .stderr(predicates::str::contains("local commits"));
}

#[test]
fn already_fresh_skips_fetch() {
    let env = TestEnv::new();
    let _school = env.setup_remote_school("test/school");

    // First run — triggers fetch (no FETCH_HEAD yet).
    env.ace().assert().success();

    // Second run — FETCH_HEAD is fresh, should skip fetch entirely.
    env.ace()
        .assert()
        .success()
        .stderr(predicates::str::contains("Fetch").not());
}

#[test]
fn fetches_new_changes() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Push new content to origin.
    push_to_origin(&env, &school, "skills/maverick/NEW.md", "# New content\n");
    make_stale(&school);

    env.ace().assert().success();

    // Verify new content was merged into cache.
    assert!(
        school.cache.join("skills/maverick/NEW.md").exists(),
        "new file should be fetched and merged",
    );
}
