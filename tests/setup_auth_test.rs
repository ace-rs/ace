mod common;

use common::TestEnv;

fn school_transport(ssh_access: bool, https_access: bool) -> TestEnv {
    let env = TestEnv::new();
    env.git_init();
    env.write_executable(
        "bin/git",
        &format!(
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$HOME/git-calls"
case "$*" in
  *ls-remote*git@github.com*) exit {ssh_status} ;;
  *ls-remote*https://github.com*) exit {https_status} ;;
  *clone*)
    for destination do :; done
    mkdir -p "$destination/.git"
    printf 'name = "School"\n' > "$destination/school.toml"
    exit 0
    ;;
  *) exit 1 ;;
esac
"#,
            ssh_status = u8::from(!ssh_access),
            https_status = u8::from(!https_access),
        ),
    );
    env
}

#[test]
fn setup_selects_ssh_when_repository_is_accessible() {
    let env = school_transport(true, false);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["setup", "example/school"])
        .assert()
        .success();

    let calls = env.read_file("git-calls");
    assert!(
        calls.lines().any(
            |call| call.contains("clone") && call.contains("git@github.com:example/school.git")
        )
    );
    assert!(!calls.contains("https://github.com"));
    assert!(
        !calls.contains("fetch"),
        "setup must reuse the acquired school"
    );
}

#[test]
fn setup_uses_https_when_ssh_is_unavailable() {
    let env = school_transport(false, true);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["setup", "example/school"])
        .assert()
        .success();

    let calls = env.read_file("git-calls");
    assert!(calls.contains("ls-remote"));
    assert!(calls.lines().any(
        |call| call.contains("clone") && call.contains("https://github.com/example/school.git")
    ));
}

#[test]
fn unattended_access_failure_preserves_retryable_setup() {
    let env = school_transport(false, false);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["--yes", "setup", "example/school"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("terminal"));

    env.assert_not_exists("ace.toml");
    assert!(!env.read_file("git-calls").contains("clone"));
}

#[test]
fn setup_preserves_an_occupied_school_cache() {
    let env = school_transport(true, true);
    env.write_file("data/ace/example/school/keep.txt", "user content");
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["setup", "example/school"])
        .assert()
        .failure();

    env.assert_contains("data/ace/example/school/keep.txt", "user content");
    env.assert_not_exists("ace.toml");
}
