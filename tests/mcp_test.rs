mod common;

use common::TestEnv;

// -- ace mcp reset --

const SCHOOL_TOML_TWO_SERVERS: &str = r#"
name = "test-school"

[[mcp]]
name = "linear"
url = "https://mcp.linear.app/mcp"

[[mcp]]
name = "github"
url = "https://api.githubcopilot.com/mcp/"
"#;

#[test]
fn mcp_reset_removes_all_registered() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    env.write_flaude_mcp_list(&["linear", "github"]);

    env.ace().args(["mcp", "reset"]).assert().success();

    // After reset, both should be removed from the list
    let list_content = env.read_file(".flaude-mcp-list");
    assert!(!list_content.contains("linear"), "linear should be removed");
    assert!(!list_content.contains("github"), "github should be removed");
}

#[test]
fn mcp_reset_removes_specific_server() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    env.write_flaude_mcp_list(&["linear", "github"]);

    env.ace().args(["mcp", "reset", "linear"]).assert().success();

    // Only linear should be removed
    let list_content = env.read_file(".flaude-mcp-list");
    assert!(!list_content.contains("linear"), "linear should be removed");
    assert!(list_content.contains("github"), "github should remain");
}

#[test]
fn mcp_reset_noop_when_nothing_registered() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    // No flaude-mcp-list → nothing registered

    env.ace().args(["mcp", "reset"]).assert().success();
}

#[test]
fn mcp_clear_is_alias_for_reset() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    env.write_flaude_mcp_list(&["linear"]);

    env.ace().args(["mcp", "clear"]).assert().success();

    let list_content = env.read_file(".flaude-mcp-list");
    assert!(!list_content.contains("linear"), "linear should be removed");
}

// -- ace mcp check --

#[test]
fn mcp_check_reports_registered_servers() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    env.write_flaude_mcp_list(&["linear", "github"]);

    let output = env.ace().args(["mcp", "check"]).output().expect("should run");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("linear"), "should report linear status");
    assert!(combined.contains("github"), "should report github status");
}

#[test]
fn mcp_check_reports_missing_servers() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    // Only linear is registered
    env.write_flaude_mcp_list(&["linear"]);

    let output = env.ace().args(["mcp", "check"]).output().expect("should run");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("github"), "should mention github as missing");
}

// -- ace mcp (default) --

#[test]
fn mcp_default_registers_missing_and_checks() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    // Nothing registered yet

    env.ace().args(["mcp"]).assert().success();

    // Should have registered both servers
    let records = env.read_flaude_mcp_records();
    assert_eq!(records.len(), 2, "should register both missing servers");
}

const SCHOOL_TOML_OAUTH: &str = r#"
name = "test-school"

[[mcp]]
name = "linear"
url = "https://mcp.linear.app/mcp"
"#;

const SCHOOL_TOML_WITH_HEADERS: &str = r#"
name = "test-school"

[[mcp]]
name = "sentry"
url = "https://mcp.sentry.dev/sse"

[mcp.headers]
Authorization = "Bearer test-token-123"
"#;

#[test]
fn mcp_register_oauth_server() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_OAUTH);

    env.ace().assert().success();

    let records = env.read_flaude_mcp_records();
    assert_eq!(records.len(), 1, "should register one server");
    assert_eq!(records[0].name, "linear");
    assert_eq!(records[0].url, "https://mcp.linear.app/mcp");
    assert!(records[0].headers.is_empty(), "OAuth server has no headers");
}

#[test]
fn mcp_register_with_headers() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_WITH_HEADERS);

    env.ace().assert().success();

    let records = env.read_flaude_mcp_records();
    assert_eq!(records.len(), 1, "should register one server");
    assert_eq!(records[0].name, "sentry");
    assert!(
        records[0]
            .headers
            .iter()
            .any(|h| h == "Authorization: Bearer test-token-123"),
        "should include auth header, got: {:?}",
        records[0].headers
    );
}

#[test]
fn mcp_skips_already_registered_servers() {
    let env = TestEnv::new();
    env.setup_flaude_school(SCHOOL_TOML_TWO_SERVERS);
    // Pre-register one of the two servers.
    env.write_flaude_mcp_list(&["linear"]);

    env.ace().args(["mcp"]).assert().success();

    // Only the missing server should get an mcp_add record.
    let records = env.read_flaude_mcp_records();
    assert_eq!(records.len(), 1, "should only register the missing server");
    assert_eq!(records[0].name, "github", "should register github, not linear");
}
