mod common;

use common::TestEnv;
use predicates::prelude::*;

fn installed_adapter(env: &TestEnv) {
    env.write_dogfood_school("name = \"pi-school\"\nbackend = \"pi\"\n");
    env.mkdir("adapter");
    env.write_executable(
        "bin/pi",
        r#"#!/bin/sh
test "$1" = list && test "$2" = --no-approve && test "$PI_OFFLINE" = 1 || exit 91
printf 'User packages:\n  npm:pi-mcp-adapter\n    %s/adapter\n' "$HOME"
"#,
    );
}

#[test]
fn missing_adapter_manifest_is_an_unavailable_installation() {
    let env = TestEnv::new();
    installed_adapter(&env);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["-p", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("install npm:pi-mcp-adapter@3"));
    env.assert_not_exists(".pi");
    env.assert_not_exists(".gitignore");
}

#[test]
fn adapter_v2_explains_the_supported_version_and_repair() {
    let env = TestEnv::new();
    installed_adapter(&env);
    env.write_file(
        "adapter/package.json",
        r#"{"name":"pi-mcp-adapter","version":"2.9.0"}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["-p", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(">=3.0.0 and <4.0.0"))
        .stderr(predicate::str::contains("install npm:pi-mcp-adapter@3"));
}

#[test]
fn malformed_manifest_is_a_verification_failure_not_absence() {
    let env = TestEnv::new();
    installed_adapter(&env);
    env.write_file("adapter/package.json", "not json");
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["-p", "hello"])
        .assert()
        .code(3)
        .stderr(predicate::str::contains("Invalid pi-mcp-adapter manifest"));
}
