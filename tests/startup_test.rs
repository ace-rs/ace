mod common;

use common::TestEnv;

const LEGACY_INDEX: &str = "cache/ace/index.toml";
const NEW_INDEX: &str = "data/ace/index.toml";

const SEED_LEGACY: &str = r#"[[school]]
specifier = "ace-rs/school"
repo = "ace-rs/school"
"#;

/// Run `ace paths` against the sandboxed env without the hidden `ace setup .`
/// that `setup_embedded` does — that extra invocation would consume the
/// migration before our real test invocation.
/// Run `ace paths` purely for its startup side effects (index migration +
/// stray-cache warning). We don't assert success — `ace paths` exits non-zero
/// when no `ace.toml` is configured, but the startup hooks we're testing run
/// before that. Using `setup_embedded` would run a hidden `ace setup .` that
/// eats the migration before our test invocation gets a chance.
fn run_ace_paths(env: &TestEnv) -> std::process::Output {
    env.ace().args(["paths"]).output().expect("ace paths")
}

#[test]
fn startup_migrates_legacy_index_toml_to_data_dir() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);

    run_ace_paths(&env);

    env.assert_exists(NEW_INDEX);
    let migrated = env.read_file(NEW_INDEX);
    assert!(
        migrated.contains("ace-rs/school"),
        "migrated index should preserve specifier; got {migrated:?}",
    );
}

#[test]
fn startup_removes_legacy_index_toml_once_adopted() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);

    run_ace_paths(&env);

    env.assert_exists(NEW_INDEX);
    env.assert_not_exists(LEGACY_INDEX);
}

#[test]
fn startup_stamps_the_layout_version() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);

    run_ace_paths(&env);

    let migrated = env.read_file(NEW_INDEX);
    assert!(
        migrated.contains("layout_version"),
        "migrated index should record the layout it was brought up to; got {migrated:?}",
    );
}

#[test]
fn startup_sweeps_flat_layout_import_clones() {
    let env = TestEnv::new();
    env.write_file(NEW_INDEX, SEED_LEGACY);
    env.mkdir("cache/ace/imports/owner/repo");
    env.mkdir("cache/ace/imports/github.com/owner/repo");

    run_ace_paths(&env);

    env.assert_not_exists("cache/ace/imports/owner");
    env.assert_exists("cache/ace/imports/github.com/owner/repo");
}

#[test]
fn startup_keeps_a_legacy_clone_that_holds_unpushed_work() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);
    env.mkdir("cache/ace/acme/school");
    env.write_file("cache/ace/acme/school/notes.md", "unsaved\n");
    env.git_init_at("cache/ace/acme/school");

    run_ace_paths(&env);

    env.assert_exists("cache/ace/acme/school/notes.md");
}

#[test]
fn startup_migration_is_silent_on_the_second_run() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);

    run_ace_paths(&env);
    let output = run_ace_paths(&env);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.to_lowercase().contains("migrat"),
        "migration should announce once, not on every startup; got stderr={stderr:?}",
    );
}

#[test]
fn startup_prefers_new_index_when_both_exist() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);
    env.write_file(
        NEW_INDEX,
        r#"[[school]]
specifier = "acme/school"
repo = "acme/school"
"#,
    );

    run_ace_paths(&env);

    let new_content = env.read_file(NEW_INDEX);
    assert!(
        new_content.contains("acme/school") && !new_content.contains("ace-rs/school"),
        "new index should be untouched when already present; got {new_content:?}",
    );
}

#[test]
fn startup_prints_migration_hint_when_legacy_index_migrates() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);

    let output = run_ace_paths(&env);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.to_lowercase().contains("migrat") && stderr.contains("index.toml"),
        "expected migration hint mentioning index.toml; got stderr={stderr:?}",
    );
}

#[test]
fn startup_no_migration_hint_when_no_legacy() {
    let env = TestEnv::new();

    let output = run_ace_paths(&env);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.to_lowercase().contains("migrat"),
        "should not mention migration when nothing migrated; got stderr={stderr:?}",
    );
}

#[test]
fn startup_keeps_the_new_index_and_still_clears_the_legacy_one() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);
    env.write_file(
        NEW_INDEX,
        r#"[[school]]
specifier = "acme/school"
repo = "acme/school"
"#,
    );

    run_ace_paths(&env);

    let new_content = env.read_file(NEW_INDEX);
    assert!(
        new_content.contains("acme/school") && !new_content.contains("ace-rs/school"),
        "the live index wins; got {new_content:?}",
    );
    env.assert_not_exists(LEGACY_INDEX);
}

#[test]
fn startup_rebuilds_an_unparseable_index() {
    let env = TestEnv::new();
    env.write_file(NEW_INDEX, "this is not toml {{{\n");

    let output = run_ace_paths(&env);

    let rebuilt = env.read_file(NEW_INDEX);
    assert!(
        rebuilt.contains("layout_version") && !rebuilt.contains("not toml"),
        "unreadable index should be discarded and rebuilt; got {rebuilt:?}",
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("index.toml"),
        "rebuild should warn and name the file; got stderr={stderr:?}",
    );
    assert_eq!(
        stderr.matches("rebuilding it").count(),
        1,
        "the index is read twice per run, but a rebuild is one event; got stderr={stderr:?}",
    );
}

#[test]
fn startup_continues_when_the_legacy_index_cannot_be_adopted() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, "this is not toml {{{\n");

    run_ace_paths(&env);

    env.assert_not_exists(LEGACY_INDEX);
    let rebuilt = env.read_file(NEW_INDEX);
    assert!(
        rebuilt.contains("layout_version"),
        "an unadoptable legacy index should not stop the migration; got {rebuilt:?}",
    );
}

#[test]
fn startup_tells_the_user_to_delete_what_it_kept() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);
    env.mkdir("cache/ace/acme/school");
    env.write_file("cache/ace/acme/school/notes.md", "unsaved\n");
    env.git_init_at("cache/ace/acme/school");

    let output = run_ace_paths(&env);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cache/ace/acme/school") && stderr.contains("delete it"),
        "a kept directory should name itself and say to delete it; got stderr={stderr:?}",
    );
}

#[test]
fn startup_does_nothing_when_neither_index_exists() {
    let env = TestEnv::new();

    run_ace_paths(&env);

    env.assert_not_exists(NEW_INDEX);
    env.assert_not_exists(LEGACY_INDEX);
}
