mod common;

use common::TestEnv;
use predicates::prelude::*;
use serde_json::Value;

fn fixture() -> TestEnv {
    let env = TestEnv::new();
    env.write_dogfood_school("name = \"pi-school\"\nbackend = \"pi\"\n[[mcp]]\nname = \"sample\"\nurl = \"https://example.com/mcp\"\n");
    env.write_file(
        "adapter/package.json",
        r#"{"name":"pi-mcp-adapter","version":"3.0.0"}"#,
    );
    env.write_executable(
        "bin/pi",
        r#"#!/bin/sh
printf '%s\n' "$PWD:$*" >> "$HOME/pi-record"
case "$1" in
list)
    printf 'User packages:\n  npm:pi-mcp-adapter@3.0.0\n    %s/adapter\n' "$HOME"
    exit 0
    ;;
--print)
    cat > "$HOME/health-prompt"
    printf 'Pi diagnostic remains visible\n' >&2
    cat "$HOME/health-response"
    exit "${HEALTH_EXIT:-0}"
    ;;
esac
exit 91
"#,
    );
    env
}

#[test]
fn registers_only_in_selected_user_scope_and_preserves_unknown_content() {
    let env = fixture();
    env.write_file("agent/mcp-adapter.json", r#"{"settings":{"directTools":"search"},"unknown":{"keep":true},"mcpServers":{"other":{"url":"https://other","futureOption":2}}}"#);
    env.ace_with_path_prefix(&env.path("bin"))
        .env("PI_CODING_AGENT_DIR", env.path("agent"))
        .args(["mcp", "register", "sample"])
        .assert()
        .success();

    let document: Value =
        serde_json::from_str(&env.read_file("agent/mcp-adapter.json")).expect("JSON");
    assert_eq!(document["unknown"]["keep"], true);
    assert_eq!(document["settings"]["directTools"], "search");
    assert_eq!(document["mcpServers"]["other"]["futureOption"], 2);
    assert_eq!(
        document["mcpServers"]["sample"]["url"],
        "https://example.com/mcp"
    );
    env.assert_not_exists(".mcp.json");
    env.assert_not_exists(".pi/agent/mcp-adapter.json");
}

#[test]
fn project_registration_is_preserved_and_prevents_user_scope_addition() {
    let env = fixture();
    let project = r#"{"mcpServers":{"sample":{"url":"https://project","disabled":true}}}"#;
    env.write_file(".pi/mcp-adapter.json", project);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "register", "sample"])
        .assert()
        .success()
        .stderr(predicate::str::contains("disabled"));

    assert_eq!(env.read_file(".pi/mcp-adapter.json"), project);
    env.assert_not_exists(".pi/agent/mcp-adapter.json");
}

#[test]
fn removing_user_entry_reports_remaining_highest_layer() {
    let env = fixture();
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"keep":true,"mcpServers":{"sample":{"url":"https://user"}}}"#,
    );
    env.write_file(
        ".mcp.json",
        r#"{"mcpServers":{"sample":{"url":"https://shared-project"}}}"#,
    );
    env.write_file(
        ".pi/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"disabled":true}}}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "unregister", "sample"])
        .assert()
        .success()
        .stderr(predicate::str::contains(".pi/mcp-adapter.json"));

    let document: Value =
        serde_json::from_str(&env.read_file(".pi/agent/mcp-adapter.json")).expect("JSON");
    assert!(document["mcpServers"].get("sample").is_none());
    assert_eq!(document["keep"], true);
    assert!(env.read_file(".mcp.json").contains("sample"));
}

#[test]
fn malformed_layer_blocks_registration_without_touching_exclusions() {
    let env = fixture();
    env.write_file(".agents/mcp.json", "{corrupt");
    env.write_file("ace.local.toml", "exclude_mcp = [\"sample\"]\n");
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "register", "sample"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(".agents/mcp.json"));

    env.assert_not_exists(".pi/agent/mcp-adapter.json");
    assert_eq!(
        env.read_file("ace.local.toml"),
        "exclude_mcp = [\"sample\"]\n"
    );
    assert_eq!(env.read_file(".agents/mcp.json"), "{corrupt");
}

#[test]
fn explicit_config_replaces_global_adapter_layer_but_not_project_override() {
    let env = fixture();
    env.write_file("ace.toml", "school = \".\"\nbackend = \"custom\"\n[backends.custom]\nkind = \"pi\"\ncmd = [\"{{ project_dir }}/bin/pi\", \"--mcp-config\", \"selected.json\"]\n");
    env.write_executable(
        "bin/pi",
        r#"#!/bin/sh
shift 2
printf 'User packages:\n  npm:pi-mcp-adapter@3.0.0\n    %s/adapter\n' "$HOME"
"#,
    );
    env.write_file(".pi/agent/mcp-adapter.json", "{ignored-corrupt");
    env.write_file(
        "selected.json",
        r#"{"mcpServers":{"sample":{"url":"https://selected"}},"keep":1}"#,
    );
    env.write_file(
        ".mcp.json",
        r#"{"mcpServers":{"sample":{"url":"https://project"}}}"#,
    );
    env.ace()
        .args(["mcp", "unregister", "sample"])
        .assert()
        .success()
        .stderr(predicate::str::contains(".mcp.json"));

    assert_eq!(
        env.read_file(".pi/agent/mcp-adapter.json"),
        "{ignored-corrupt"
    );
    let document: Value = serde_json::from_str(&env.read_file("selected.json")).expect("JSON");
    assert!(document["mcpServers"].get("sample").is_none());
    assert_eq!(document["keep"], 1);
}

#[test]
fn ancestor_discovery_uses_global_boundary_and_nearest_layer() {
    let env = fixture();
    env.write_file("work/project/ace.toml", "backend = \"pi\"\n");
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        &serde_json::json!({"settings":{"ancestorConfigRoots":[env.root()]}}).to_string(),
    );
    env.write_file(
        ".mcp.json",
        r#"{"mcpServers":{"sample":{"url":"https://outer"},"outer":{"url":"https://outer"}}}"#,
    );
    env.write_file(
        "work/.pi/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://near"}}}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .current_dir(env.path("work/project"))
        .args(["mcp", "unregister", "sample"])
        .assert()
        .success()
        .stderr(predicate::str::contains("work/.pi/mcp-adapter.json"));
    env.ace_with_path_prefix(&env.path("bin"))
        .current_dir(env.path("work/project"))
        .args(["mcp"])
        .assert()
        .success()
        .stdout(predicate::str::contains("outer\tnot in linked school"));
}

#[test]
fn project_cannot_enable_ancestor_discovery() {
    let env = fixture();
    env.write_file("work/project/ace.toml", "backend = \"pi\"\n");
    env.write_file(
        "work/project/.pi/mcp-adapter.json",
        &serde_json::json!({"settings":{"ancestorConfigRoots":[env.root()]}}).to_string(),
    );
    env.write_file(
        "work/.mcp.json",
        r#"{"mcpServers":{"ancestor":{"url":"https://outside"}}}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .current_dir(env.path("work/project"))
        .args(["mcp"])
        .assert()
        .success()
        .stdout(predicate::str::contains("ancestor").not());
}

#[test]
fn nearer_ancestor_symlink_retains_its_precedence_and_provenance() {
    let env = fixture();
    env.write_file("work/project/ace.toml", "backend = \"pi\"\n");
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        &serde_json::json!({"settings":{"ancestorConfigRoots":[env.root()]}}).to_string(),
    );
    env.write_file(
        ".mcp.json",
        r#"{"mcpServers":{"sample":{"url":"https://ancestor"}}}"#,
    );
    env.symlink(".mcp.json", "work/.mcp.json");
    env.ace_with_path_prefix(&env.path("bin"))
        .current_dir(env.path("work/project"))
        .args(["mcp", "unregister", "sample"])
        .assert()
        .success()
        .stderr(predicate::str::contains("work/.mcp.json"));
}

#[test]
fn user_config_symlink_is_preserved() {
    let env = fixture();
    env.write_file("owned/adapter.json", r#"{"mcpServers":{},"keep":true}"#);
    env.mkdir(".pi/agent");
    env.symlink("owned/adapter.json", ".pi/agent/mcp-adapter.json");
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "register", "sample"])
        .assert()
        .success();

    env.assert_symlink(".pi/agent/mcp-adapter.json", "owned/adapter.json");
    assert!(env.read_file("owned/adapter.json").contains("sample"));
}

#[test]
fn selected_user_file_alias_does_not_leave_a_stale_project_registration() {
    let env = fixture();
    env.write_file("ace.toml", "school = \".\"\nbackend = \"custom\"\n[backends.custom]\nkind = \"pi\"\ncmd = [\"{{ project_dir }}/bin/pi\", \"--mcp-config\", \"selected.json\"]\n");
    env.write_executable(
        "bin/pi",
        r#"#!/bin/sh
printf 'User packages:\n  npm:pi-mcp-adapter@3.0.0\n    %s/adapter\n' "$HOME"
"#,
    );
    env.write_file(
        ".pi/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://selected"}}}"#,
    );
    env.symlink(".pi/mcp-adapter.json", "selected.json");
    env.ace()
        .args(["mcp", "unregister", "sample"])
        .assert()
        .success()
        .stderr(predicate::str::contains("remains registered").not());
    let document: Value =
        serde_json::from_str(&env.read_file(".pi/mcp-adapter.json")).expect("JSON");
    assert!(document["mcpServers"].get("sample").is_none());
}

#[test]
fn health_exercises_resolved_pi_and_retains_diagnostics() {
    let env = fixture();
    env.write_file("ace.toml", "school = \".\"\nbackend = \"pi\"\n[backends.pi]\nmodel = \"fixture-model\"\neffort = \"low\"\n");
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://sample"}}}"#,
    );
    env.write_file(
        "health-response",
        r#"{"statuses":[{"name":"sample","ok":true}]}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "check"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"statuses\""))
        .stderr(predicate::str::contains("Pi diagnostic remains visible"));

    let record = env.read_file("pi-record");
    assert!(record.contains("--model fixture-model"), "{record}");
    assert!(record.contains("--thinking low"), "{record}");
    let prompt = env.read_file("health-prompt");
    assert!(prompt.contains("successful live connection"), "{prompt}");
    assert!(
        prompt.contains("Do not call any discovered server tool"),
        "{prompt}"
    );
}

#[test]
fn incomplete_health_result_is_failure_not_registered_success() {
    let env = fixture();
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://sample"}}}"#,
    );
    env.write_file("health-response", r#"{"statuses":[]}"#);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("omitted server 'sample'"));
}

#[test]
fn unhealthy_server_makes_health_command_fail() {
    let env = fixture();
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://sample"}}}"#,
    );
    env.write_file(
        "health-response",
        r#"{"statuses":[{"name":"sample","ok":false}]}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "check"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("sample (unhealthy)"));
}

#[test]
fn unsupported_imports_are_errors_instead_of_empty_inventory() {
    let env = fixture();
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"imports":["codex"],"mcpServers":{}}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot inventory imports"));
}

#[test]
fn package_defined_servers_do_not_disappear_from_inventory_silently() {
    let env = fixture();
    env.write_file(
        "adapter/package.json",
        r#"{"name":"pi-mcp-adapter","version":"3.0.0","pi":{"mcp":"mcp.json"}}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("package-declared pi.mcp"));
}

#[test]
fn nonzero_pi_health_exit_preserves_output_and_fails() {
    let env = fixture();
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://sample"}}}"#,
    );
    env.write_file(
        "health-response",
        r#"{"statuses":[{"name":"sample","ok":true}]}"#,
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .env("HEALTH_EXIT", "23")
        .args(["mcp", "check"])
        .assert()
        .failure()
        .stdout(predicate::str::contains("\"statuses\""))
        .stderr(predicate::str::contains("Pi diagnostic remains visible"))
        .stderr(predicate::str::contains("exited with"));
}
