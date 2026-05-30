// End-to-end exit-code contract. See docs/decisions/2026-05-30-exit-codes.md.
// 1 = Usage, 2 = Unavailable, 3 = Operational. (130 = Cancelled is unit-tested
// in src/cmd/mod.rs — it needs a prompt abort no headless run can trigger.)

mod common;

use common::TestEnv;

#[test]
fn bad_flag_combo_exits_usage() {
    let env = TestEnv::new();

    // Conflicting scope flags are rejected before any subcommand dispatch.
    env.ace().args(["--user", "--project", "paths"]).assert().code(1);
}

#[test]
fn unknown_config_key_exits_usage() {
    let env = TestEnv::new();
    env.setup_embedded("maverick");

    env.ace().args(["config", "get", "no-such-key"]).assert().code(1);
}

#[test]
fn not_in_git_repo_exits_unavailable() {
    let env = TestEnv::new();
    env.write_file("school.toml", "name = \"test-school\"\n");

    // Precondition (a git repo) is absent — Unavailable, not Usage.
    env.ace().args(["setup", "."]).assert().code(2);
}

#[test]
fn missing_school_exits_unavailable() {
    let env = TestEnv::new();
    env.git_init();

    // No ace.toml at all — nothing configured to pull.
    env.ace().args(["pull"]).assert().code(2);
}

#[test]
fn already_set_up_exits_usage() {
    let env = TestEnv::new();
    env.setup_embedded("iceman");

    // Re-running setup is a user mistake against current state — Usage.
    env.ace().args(["setup", "."]).assert().code(1);
}

#[test]
fn clone_failure_exits_operational() {
    let env = TestEnv::new();
    env.git_init();
    env.redirect_to_invalid("fake/repo");

    // Valid invocation, precondition met, but the clone itself fails.
    env.ace().args(["setup", "fake/repo"]).assert().code(3);
}
