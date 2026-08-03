mod common;

use common::TestEnv;
use predicates::prelude::*;

// Linked-school resolution goes through the ace.toml specifier exclusively;
// school.toml is content at the resolved root, never a location marker.
// Contract: docs/spec/school/overview.md (Linked-School Resolution case matrix).

/// Case 4/5: local specifier `.` with no school.toml at the resolved root —
/// NotInitialized, not a silent scan of an empty root.
#[test]
fn local_specifier_uninitialized_errors_not_initialized() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "school = \".\"\nbackend = \"flaude\"\n");

    env.ace()
        .args(["skills"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("school not initialized"));
}

/// Dogfood: `school = "."` plus a workdir school.toml resolves to the workdir
/// via the specifier — not via marker-file detection.
#[test]
fn embedded_specifier_resolves_to_workdir() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "school = \".\"\nbackend = \"flaude\"\n");
    env.write_file("school.toml", "name = \"x\"\n");

    env.ace().args(["skills"]).assert().success();
}

/// Case 2: a bare school.toml without any ace.toml does not short-circuit —
/// resolution always goes through the tree, which fails first (intent
/// unknowable), and neither school hint fires.
#[test]
fn workdir_school_toml_without_ace_toml_errors() {
    let env = TestEnv::new();
    env.write_file("school.toml", "name = \"x\"\n");

    env.ace()
        .args(["skills"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("ace school init").not());
}

/// A present-but-malformed school.toml errors loudly instead of resolving as
/// "no school" — a typo must not make the school's config vanish silently.
#[test]
fn malformed_school_toml_errors() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "school = \".\"\nbackend = \"flaude\"\n");
    env.write_file("school.toml", "name = [not toml\n");

    env.ace().args(["skills"]).assert().failure();
}

/// Case 3: ace.toml without a `school = ...` specifier — NoSpecifier.
#[test]
fn no_specifier_errors_no_school_configured() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "backend = \"flaude\"\n");

    env.ace()
        .args(["skills"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicates::str::contains("no school configured"));
}
