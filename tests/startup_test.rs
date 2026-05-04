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
    env.ace()
        .args(["paths"])
        .output()
        .expect("ace paths")
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
fn startup_leaves_legacy_index_toml_in_place() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);

    run_ace_paths(&env);

    env.assert_exists(LEGACY_INDEX);
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
fn startup_no_migration_hint_when_new_already_exists() {
    let env = TestEnv::new();
    env.write_file(LEGACY_INDEX, SEED_LEGACY);
    env.write_file(NEW_INDEX, SEED_LEGACY);

    let output = run_ace_paths(&env);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.to_lowercase().contains("migrat"),
        "should not re-announce migration on subsequent startups; got stderr={stderr:?}",
    );
}

#[test]
fn startup_does_nothing_when_neither_index_exists() {
    let env = TestEnv::new();

    run_ace_paths(&env);

    env.assert_not_exists(NEW_INDEX);
    env.assert_not_exists(LEGACY_INDEX);
}
