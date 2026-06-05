mod common;

use common::TestEnv;

#[test]
fn paths_lists_all_keys() {
    let env = TestEnv::new();
    env.setup_embedded("top-gun");

    let output = env.ace().args(["paths"]).output().expect("ace paths");

    assert!(output.status.success(), "ace paths should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(stdout.contains("project\t"), "should list project");
    assert!(stdout.contains("cache\t"), "should list cache");
    assert!(stdout.contains("school\t"), "should list school");
}

#[test]
fn paths_single_key() {
    let env = TestEnv::new();
    env.setup_embedded("top-gun");

    let output = env
        .ace()
        .args(["paths", "project"])
        .output()
        .expect("ace paths project");

    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains('\t'),
        "single key should not have tab separator"
    );
    assert!(!stdout.contains("cache"), "should not list other keys");
}

#[test]
fn paths_school_key() {
    let env = TestEnv::new();
    env.setup_embedded("top-gun");

    let output = env
        .ace()
        .args(["paths", "school"])
        .output()
        .expect("ace paths school");

    assert!(output.status.success(), "'school' key should resolve");
}

#[test]
fn paths_unknown_key() {
    let env = TestEnv::new();
    env.setup_embedded("top-gun");

    env.ace()
        .args(["paths", "nonexistent"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unknown key"));
}
