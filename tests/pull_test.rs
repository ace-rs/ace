mod common;

use common::{RemoteSchool, TestEnv};

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

// -- Tests --

#[test]
fn pull_fetches_without_cooldown() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    // First run — creates FETCH_HEAD via normal ace flow.
    env.ace().assert().success();

    // Push new content to origin while FETCH_HEAD is still fresh.
    push_to_origin(&env, &school, "skills/maverick/NEW.md", "# New\n");

    // `ace pull` should fetch even though FETCH_HEAD is fresh.
    env.ace().args(["pull"]).assert().success();

    // Verify new content was fetched and merged.
    assert!(
        school.cache.join("skills/maverick/NEW.md").exists(),
        "pull should fetch new file despite fresh FETCH_HEAD",
    );
}

#[test]
fn pull_no_school_fails() {
    let env = TestEnv::new();
    env.git_init();
    env.write_file("ace.toml", "backend = \"flaude\"\n");

    env.ace().args(["pull"]).assert().failure();
}

// -- Stale-index self-heal --
//
// When the index.toml entry references a clone that no longer exists on disk
// (e.g. user deleted ~/.local/share/ace, or upgraded from a pre-XDG layout
// where the clone never migrated), ace should re-clone instead of erroring
// with "clone failed: school not installed".

#[test]
fn bare_ace_reclones_when_clone_dir_missing() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    std::fs::remove_dir_all(&school.cache).expect("remove clone dir");

    env.ace().assert().success();

    assert!(
        school.cache.join(".git").exists(),
        "ace should have re-cloned the school",
    );
}

#[test]
fn bare_ace_reclones_when_git_dir_missing() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    std::fs::remove_dir_all(school.cache.join(".git")).expect("remove .git");

    env.ace().assert().success();

    assert!(
        school.cache.join(".git").exists(),
        "ace should have re-cloned the school",
    );
}

#[test]
fn ace_pull_reclones_when_clone_missing() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    std::fs::remove_dir_all(&school.cache).expect("remove clone dir");

    env.ace().args(["pull"]).assert().success();

    assert!(
        school.cache.join(".git").exists(),
        "ace pull should have re-cloned the school",
    );
}

#[test]
fn self_heal_does_not_duplicate_index_entry() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test/school");

    std::fs::remove_dir_all(&school.cache).expect("remove clone dir");

    env.ace().assert().success();

    let index = std::fs::read_to_string(env.path("cache/ace/index.toml"))
        .expect("read index.toml");
    let count = index.matches("specifier = \"test/school\"").count();
    assert_eq!(count, 1, "index should have exactly one entry, got:\n{index}");
}
