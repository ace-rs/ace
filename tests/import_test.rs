mod common;

use common::TestEnv;

#[test]
fn import_no_school_context() {
    let env = TestEnv::new();
    env.git_init();

    // No school.toml, no ace.toml — hard error naming both bootstrap routes.
    env.ace()
        .args(["import", "owner/repo", "--skill", "my-skill"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("ace school init"))
        .stderr(predicates::str::contains("ace setup"));
}

#[test]
fn import_clone_failure_invalid_source() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");
    env.redirect_to_invalid("nonexistent-owner-xxxxx/nonexistent-repo-xxxxx");

    // Source that cannot be cloned — redirected to a local nonexistent path.
    env.ace()
        .args([
            "import",
            "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx",
            "--skill",
            "my-skill",
        ])
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
    env.ace().args(["import"]).assert().failure();
}

#[test]
fn import_from_local_school_context() {
    let env = TestEnv::new();
    env.git_init();

    // School repo context (dogfood pair present) but invalid remote source.
    env.write_dogfood_school("name = \"my-school\"\n");
    env.mkdir("skills");
    env.redirect_to_invalid("nonexistent-owner-xxxxx/nonexistent-repo-xxxxx");

    // The source is invalid, so clone fails — but this verifies that import
    // correctly resolves the school context via ace.toml's specifier.
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
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");
    env.redirect_to_invalid("nonexistent-owner-xxxxx/nonexistent-repo-xxxxx");

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
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");
    env.redirect_to_invalid("nonexistent-owner-xxxxx/nonexistent-repo-xxxxx");

    // Import should still work to find school context (school.toml check
    // doesn't require git), but clone will fail on the remote source.
    env.ace()
        .args([
            "import",
            "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx",
            "--skill",
            "x",
        ])
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
    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
skill = "existing-skill"
source = "some-owner/some-repo"
"#,
    );
    env.mkdir("skills/existing-skill");
    env.write_file("skills/existing-skill/SKILL.md", "# Existing\n");
    env.redirect_to_invalid("nonexistent-owner-xxxxx/nonexistent-repo-xxxxx");

    // Importing a new skill from an invalid source fails at clone.
    // Verifies that having existing imports doesn't break the import flow.
    env.ace()
        .args([
            "import",
            "nonexistent-owner-xxxxx/nonexistent-repo-xxxxx",
            "--skill",
            "new-skill",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("git clone"));
}

#[test]
fn import_all_adds_wildcard_entry() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    // --all writes a wildcard import entry without cloning (no network needed).
    env.ace()
        .args(["import", "company/school", "--all"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "Added import: * from company/school",
        ));

    let toml = env.read_file("school.toml");
    // Canonical form per docs/spec/skills/selection.md is the plural `skills` array.
    assert!(
        toml.contains("skills = [\"*\"]"),
        "should have wildcard skills entry: {toml}"
    );
    assert!(
        toml.contains("source = \"company/school\""),
        "should have source"
    );
}

#[test]
fn import_glob_pattern_adds_entry() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args(["import", "company/school", "--skill", "*-coding"])
        .assert()
        .success()
        .stderr(predicates::str::contains(
            "Added import: *-coding from company/school",
        ));

    let toml = env.read_file("school.toml");
    assert!(
        toml.contains("skills = [\"*-coding\"]"),
        "should have glob pattern: {toml}"
    );
}

#[test]
fn import_all_duplicate_warns() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school(
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
    env.write_dogfood_school("name = \"test-school\"\n");
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
    env.write_dogfood_school("name = \"test-school\"\n");
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
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args([
            "import",
            "owner/repo",
            "--skill",
            "foo",
            "--include-experimental",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--all"));
}

#[test]
fn import_all_include_experimental_persists_flag() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args([
            "import",
            "company/school",
            "--all",
            "--include-experimental",
        ])
        .assert()
        .success();

    let toml = env.read_file("school.toml");
    assert!(
        toml.contains("include_experimental = true"),
        "missing flag in {toml}"
    );
    assert!(
        !toml.contains("include_system"),
        "include_system should not be written: {toml}"
    );
}

#[test]
fn import_all_include_both_flags_persists_both() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.ace()
        .args([
            "import",
            "company/school",
            "--all",
            "--include-experimental",
            "--include-system",
        ])
        .assert()
        .success();

    let toml = env.read_file("school.toml");
    assert!(
        toml.contains("include_experimental = true"),
        "missing experimental flag: {toml}"
    );
    assert!(
        toml.contains("include_system = true"),
        "missing system flag: {toml}"
    );
}

// -- end-to-end import with real git (PROD9-75) --

#[test]
fn import_explicit_skill_resolves_from_experimental_tier() {
    // Reproduces the original bug: shell lives in skills/.experimental/ only.
    // Before PROD9-75, ACE skipped all hidden dirs and reported "no skills found".
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin(
        "dot/skills",
        &["skills/.experimental/shell", "skills/.curated/react"],
    );

    env.ace()
        .args(["import", "dot/skills", "--skill", "shell"])
        .assert()
        .success();

    env.assert_exists("skills/shell/SKILL.md");
    env.assert_contains("school.toml", "skills = [\"shell\"]");
    env.assert_contains("school.toml", "source = \"dot/skills\"");
}

#[test]
fn import_explicit_inadmissible_skill_skips_and_fails() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("bad/skills", &["skills/bad\u{202E}name"]);

    // Matches `ace school pull`: skip the bad skill, warn, exit non-zero with
    // the same RejectedImports code. See decision § Resolved Follow-Ups.
    env.ace()
        .args(["import", "bad/skills", "--skill", "bad\u{202E}name"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("skipping inadmissible skill"))
        .stderr(predicates::str::contains("skipped 1 inadmissible skill"));

    env.assert_not_exists("skills/bad\u{202E}name/SKILL.md");
    env.assert_not_contains("school.toml", "bad\u{202E}name");
}

#[test]
fn import_all_defaults_to_curated_tier_only() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin(
        "dot/skills",
        &[
            "skills/.curated/react",
            "skills/.experimental/shell",
            "skills/.system/skill-creator",
        ],
    );

    // --all without --include-* flags should record a wildcard entry only.
    env.ace()
        .args(["import", "dot/skills", "--all"])
        .assert()
        .success();

    // The actual expansion happens on school update.
    env.ace().args(["school", "update"]).assert().success();

    env.assert_exists("skills/react/SKILL.md");
    env.assert_not_exists("skills/shell/SKILL.md");
    env.assert_not_exists("skills/skill-creator/SKILL.md");
}

#[test]
fn import_all_with_include_experimental_pulls_that_tier() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin(
        "dot/skills",
        &[
            "skills/.curated/react",
            "skills/.experimental/shell",
            "skills/.system/skill-creator",
        ],
    );

    env.ace()
        .args(["import", "dot/skills", "--all", "--include-experimental"])
        .assert()
        .success();

    env.ace().args(["school", "update"]).assert().success();

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
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("cached/source", &["skills/foo", "skills/bar"]);

    env.ace()
        .args(["import", "cached/source", "--skill", "foo"])
        .assert()
        .success();

    let cache_path = env.path("cache/ace/imports/github.com/cached/source");
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
fn pull_imports_overlapping_sources_last_wins_silently() {
    let env = TestEnv::new();
    env.git_init();

    env.setup_tiered_origin("anthropics/skills", &["skills/.system/skill-creator"]);
    env.setup_tiered_origin(
        "ace-rs/school",
        &[
            "skills/.system/skill-creator",
            "skills/.curated/other-skill",
        ],
    );

    // Both sources expose skill-creator. Last-wins: ace-rs/school's version
    // silently replaces anthropics/skills'. No shadow warning.
    env.write_dogfood_school(
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
    let summary_dupes =
        combined.matches("~skill-creator").count() + combined.matches("+skill-creator").count();
    assert_eq!(
        summary_dupes, 1,
        "skill-creator appears {summary_dupes} times in summary; expected exactly 1: {combined}"
    );

    // No shadow warning — last-wins is silent.
    assert!(
        !combined.contains("declared by both"),
        "should not emit shadow warning: {combined}"
    );

    // Both skills land.
    env.assert_exists("skills/skill-creator/SKILL.md");
    env.assert_exists("skills/other-skill/SKILL.md");
}

// Spec: docs/spec/skills/model.md § Discovery cascade.
// Nested layouts under skills/<group>/<leaf>/ are discovered via the
// recursive walk inside the canonical priority dir; identity is the
// post-strip path, written into the school at the same nested path.
#[test]
fn pull_imports_with_nested_source_layout_writes_nested_identity() {
    let env = TestEnv::new();
    env.git_init();

    env.setup_tiered_origin(
        "nested/upstream",
        &["skills/typescript/coding", "skills/rust/coding"],
    );

    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
source = "nested/upstream"
skills = ["*"]
"#,
    );
    env.mkdir("skills");

    env.ace().args(["school", "update"]).assert().success();

    // Identity is preserved through the school's storage layer.
    env.assert_exists("skills/typescript/coding/SKILL.md");
    env.assert_exists("skills/rust/coding/SKILL.md");
}

#[test]
fn pull_imports_skips_inadmissible_skill_and_fails() {
    let env = TestEnv::new();
    env.git_init();

    env.setup_tiered_origin(
        "mixed/upstream",
        &["skills/good-skill", "skills/bad\u{202E}name"],
    );

    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
source = "mixed/upstream"
skills = ["*"]
"#,
    );
    env.mkdir("skills");

    env.ace()
        .args(["school", "update"])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "skipped 1 inadmissible imported skill",
        ));

    env.assert_exists("skills/good-skill/SKILL.md");
    env.assert_not_exists("skills/bad\u{202E}name/SKILL.md");
}

// Spec: docs/spec/skills/selection.md § Cross-source merge.
// Two declarations claiming the same identity from different sources
// surface a loud collision warning. First-declared wins.
#[test]
fn pull_imports_cross_source_collision_warns() {
    let env = TestEnv::new();
    env.git_init();

    env.setup_tiered_origin("upstream/alpha", &["skills/shared"]);
    env.setup_tiered_origin("fork/beta", &["skills/shared"]);

    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
source = "upstream/alpha"
skills = ["shared"]

[[imports]]
source = "fork/beta"
skills = ["shared"]
"#,
    );
    env.mkdir("skills");

    let assert = env.ace().args(["school", "update"]).assert().success();
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&assert.get_output().stdout),
        String::from_utf8_lossy(&assert.get_output().stderr),
    );
    assert!(
        combined.contains("cross-source collision"),
        "expected collision warning, got: {combined}"
    );
    // First-declared wins; only one copy lands on disk.
    env.assert_exists("skills/shared/SKILL.md");
}

// A multi-skill source with no `--skill` needs the picker, and there is no
// terminal to pick in. Exiting 0 having imported nothing would report success
// for work never done (ux.md §7) — it must fail with a way forward instead.
#[test]
fn import_multiple_skills_without_selection_errors_when_not_interactive() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("multi/source", &["skills/foo", "skills/bar"]);

    env.ace()
        .args(["import", "multi/source"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("--skill"));

    env.assert_not_exists("skills/foo/SKILL.md");
    env.assert_not_exists("skills/bar/SKILL.md");
}

#[test]
fn import_reuses_source_cache_on_second_call() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    env.setup_tiered_origin("cached/source", &["skills/foo", "skills/bar"]);

    env.ace()
        .args(["import", "cached/source", "--skill", "foo"])
        .assert()
        .success();

    // Drop a sentinel inside the cache. A re-clone would wipe it; a fetch preserves it.
    let cache_path = env.path("cache/ace/imports/github.com/cached/source");
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

// A single-skill repo (SKILL.md at the repo root) puts the clone's `.git`
// *inside* the skill's path. That is the exact shape that used to leak a nested
// `.git` into the school and turn the skill dir into an accidental submodule.
#[test]
fn import_repo_root_skill_does_not_copy_dot_git() {
    let env = TestEnv::new();
    env.git_init();
    env.write_dogfood_school("name = \"test-school\"\n");
    env.mkdir("skills");

    setup_root_skill_origin(&env, "chakrit/lowfat-pantry");

    env.ace()
        .args(["import", "chakrit/lowfat-pantry"])
        .assert()
        .success();

    env.assert_exists("skills/lowfat-pantry/SKILL.md");
    env.assert_not_exists("skills/lowfat-pantry/.git");
}

// A pre-fix import committed a skill dir as a gitlink (an accidental submodule).
// `ace school pull` must warn and skip it — never silently rewrite the user's
// index, which they may be repairing by hand.
#[test]
fn pull_imports_warns_and_skips_broken_submodule() {
    let env = TestEnv::new();
    env.git_init();

    env.setup_tiered_origin("chakrit/lowfat-pantry", &["skills/lowfat-pantry"]);
    env.write_dogfood_school(
        r#"name = "test-school"

[[imports]]
source = "chakrit/lowfat-pantry"
skills = ["lowfat-pantry"]
"#,
    );
    env.mkdir("skills");
    commit_gitlink(&env, "skills/lowfat-pantry");

    let before = env.git_in(env.root(), &["ls-files", "--stage", "skills/lowfat-pantry"]);
    assert!(
        before.starts_with("160000 "),
        "fixture should start as a gitlink: {before}"
    );

    env.ace()
        .args(["school", "update"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("committed as a git submodule"));

    // ACE must leave the index untouched — the gitlink stays until the user
    // clears it themselves.
    let after = env.git_in(env.root(), &["ls-files", "--stage", "skills/lowfat-pantry"]);
    assert!(
        after.starts_with("160000 "),
        "ACE must not rewrite the user's index: {after}"
    );
}

/// Build a bare origin whose only skill is `SKILL.md` at the repo root, with a
/// gitconfig `insteadOf` redirect so `ace import <specifier>` clones from the
/// sandbox instead of github.com.
fn setup_root_skill_origin(env: &TestEnv, specifier: &str) {
    let origin = env.path(&format!("origins/{specifier}.git"));
    let work = env.path(&format!("_root_work_{}", specifier.replace('/', "_")));

    std::fs::create_dir_all(&origin).expect("create origin dir");
    env.git_in(
        &origin,
        &["init", "--bare", "--quiet", "--template=", "-b", "main"],
    );
    env.git_in(
        env.root(),
        &[
            "clone",
            "--quiet",
            origin.to_str().expect("origin path"),
            work.to_str().expect("work path"),
        ],
    );

    std::fs::write(work.join("SKILL.md"), "# lowfat-pantry\n").expect("write SKILL.md");
    commit_all(env, &work, "seed");
    env.git_in(&work, &["push", "--quiet"]);
    std::fs::remove_dir_all(&work).expect("remove work dir");

    append_gitconfig_redirect(env, specifier, &origin);
}

/// Commit a `160000` gitlink entry at `rel` in the env's repo, pointing at a
/// throwaway sub-repo — the broken-submodule shape a pre-fix import left behind.
fn commit_gitlink(env: &TestEnv, rel: &str) {
    let sub = env.path("_gitlink_source");
    std::fs::create_dir_all(&sub).expect("mkdir gitlink source");
    env.git_in(&sub, &["init", "--quiet", "--template=", "-b", "main"]);
    std::fs::write(sub.join("README.md"), "gitlink target\n").expect("write target");
    commit_all(env, &sub, "gitlink target");
    let head = env.git_in(&sub, &["rev-parse", "HEAD"]);

    // Stage the gitlink directly via the index (the path has no working copy),
    // then commit without `git add -A` — which would otherwise stage a deletion
    // of the absent path and unstage the gitlink.
    env.git_in(
        env.root(),
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            "160000",
            head.trim(),
            rel,
        ],
    );
    git_commit(env, env.root(), "broken gitlink");
}

fn commit_all(env: &TestEnv, dir: &std::path::Path, message: &str) {
    env.git_in(dir, &["add", "-A"]);
    git_commit(env, dir, message);
}

fn git_commit(env: &TestEnv, dir: &std::path::Path, message: &str) {
    env.git_in(
        dir,
        &[
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            message,
        ],
    );
}

fn append_gitconfig_redirect(env: &TestEnv, specifier: &str, origin: &std::path::Path) {
    let gh_url = format!("https://github.com/{specifier}.git");
    let file_url = format!("file://{}", origin.display());
    let block = format!("[url \"{file_url}\"]\n\tinsteadOf = {gh_url}\n");

    let cfg = env.path(".gitconfig");
    let existing = std::fs::read_to_string(&cfg).unwrap_or_default();
    std::fs::write(&cfg, format!("{existing}{block}")).expect("write gitconfig");
}
