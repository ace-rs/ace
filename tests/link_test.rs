mod common;

use common::TestEnv;

/// `ace link` re-links school folders without pulling.
/// Primary use case: stale/broken symlinks after school clone moves or
/// symlinks get manually deleted.
#[test]
fn link_repairs_deleted_skills_symlinks() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Initial setup links everything.
    env.ace().assert().success();
    let skills_dir = env.path(".claude/skills");
    env.assert_skills_dir_is_real(".claude/skills");

    // Delete a per-skill symlink to simulate staleness.
    let maverick_link = skills_dir.join("maverick");
    assert!(maverick_link.exists(), "maverick symlink should exist after setup");
    std::fs::remove_file(&maverick_link).expect("delete symlink");
    assert!(!maverick_link.exists(), "maverick symlink should be gone");

    // `ace link` should repair the symlink without pulling.
    env.ace().args(["link"]).assert().success();

    assert!(
        maverick_link.exists(),
        "ace link should have re-created the maverick symlink",
    );
    let target = std::fs::read_link(&maverick_link).expect("read symlink");
    assert_eq!(
        target,
        school.cache.join("skills").join("maverick"),
        "re-created symlink should point into the school clone",
    );
}

#[test]
fn link_fails_without_school() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("ace.toml", "backend = \"flaude\"\n");

    env.ace().args(["link"]).assert().failure();
}

#[test]
fn link_repairs_stale_whole_dir_symlink() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // Add a rules folder to the school so it gets linked.
    env.mkdir("data/ace/test/school/rules/lint");
    env.write_file("data/ace/test/school/rules/lint/rule.md", "# Lint\n");
    env.git_in(&school.cache, &["add", "-A"]);
    env.git_in(
        &school.cache,
        &[
            "-c", "user.email=test@test.com",
            "-c", "user.name=Test",
            "commit", "-m", "add rules",
        ],
    );

    env.ace().assert().success();

    // Break the rules symlink by pointing it at a nonexistent target.
    let rules_link = env.path(".claude/rules");
    assert!(rules_link.exists(), "rules symlink should exist after setup");
    std::fs::remove_file(&rules_link).expect("remove old symlink");
    std::os::unix::fs::symlink(env.path("nonexistent"), &rules_link)
        .expect("create stale symlink");

    env.ace().args(["link"]).assert().success();

    let target = std::fs::read_link(&rules_link).expect("read symlink");
    assert!(
        target.exists(),
        "ace link should have repointed rules to a valid target",
    );
}
