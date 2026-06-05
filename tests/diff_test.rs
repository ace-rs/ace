mod common;

use common::TestEnv;

#[test]
fn diff_clean_school_no_output() {
    let env = TestEnv::new();
    let _school = env.setup_remote_school("test-org/test-school");

    let output = env.ace().args(["diff"]).output().expect("run ace diff");

    assert!(output.status.success(), "ace diff should succeed");

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(
        stdout.contains("school-clone"),
        "should print clone path header, got:\n{stdout}"
    );

    // Clean school — no diff content beyond the header line.
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        lines.len(),
        1,
        "clean school should only have header line, got:\n{stdout}"
    );
}

#[test]
fn diff_dirty_school_shows_changes() {
    let env = TestEnv::new();
    let school = env.setup_remote_school("test-org/test-school");

    // Write a new file into the school clone to make it dirty.
    let new_file = school.cache.join("dirty.txt");
    std::fs::write(&new_file, "hello dirty\n").expect("write dirty file");

    let output = env.ace().args(["diff"]).output().expect("run ace diff");

    assert!(output.status.success(), "ace diff should succeed");

    let stdout = String::from_utf8(output.stdout).expect("stdout utf8");
    assert!(
        stdout.contains("dirty.txt"),
        "diff should mention the new file, got:\n{stdout}"
    );
    assert!(
        stdout.contains("hello dirty"),
        "diff should contain file content, got:\n{stdout}"
    );
}

#[test]
fn diff_no_school_fails() {
    let env = TestEnv::new();
    env.git_init();

    // No ace.toml, no school context — should fail.
    env.ace().args(["diff"]).assert().failure();
}
