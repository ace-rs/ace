use super::{Configuration, McpDecl, McpRemoval, resolve_path, selected_config};
use crate::backend::{Invocation, Kind};
use crate::test_fixtures::TestEnv;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

fn context(env: &TestEnv) -> Invocation {
    Invocation {
        kind: Kind::Pi,
        command: vec!["pi".into()],
        project_dir: env.root().to_path_buf(),
        env: HashMap::from([("HOME".into(), env.root().display().to_string())]),
        model: None,
        effort: None,
        extra_args: Vec::new(),
    }
}

fn configuration(context: &Invocation) -> Configuration {
    load_configuration(context, &[]).expect("load isolated adapter files")
}

fn load_configuration(context: &Invocation, packages: &[PathBuf]) -> Result<Configuration, String> {
    let home = PathBuf::from(context.env.get("HOME").expect("explicit sandbox home"));
    let agent = match context.env.get("PI_CODING_AGENT_DIR") {
        Some(path) if !path.trim().is_empty() => {
            resolve_path(path.trim(), &home, &context.project_dir)
        }
        _ => home.join(".pi/agent"),
    };
    let selected = selected_config(context, &agent)?;
    Configuration::load_documents(&context.project_dir, &home, selected, packages)
}

fn sample() -> McpDecl {
    McpDecl {
        name: "sample".into(),
        url: "https://example.com/mcp".into(),
        headers: HashMap::new(),
        instructions: String::new(),
    }
}

fn document(env: &TestEnv, path: &str) -> Value {
    serde_json::from_str(&env.read_file(path)).expect("adapter JSON")
}

#[test]
fn registers_only_in_selected_user_scope_and_preserves_unknown_content() {
    let env = TestEnv::new();
    env.write_file("agent/mcp-adapter.json", r#"{"settings":{"directTools":"search"},"unknown":{"keep":true},"mcpServers":{"other":{"url":"https://other","futureOption":2}}}"#);
    let mut context = context(&env);
    context.env.insert(
        "PI_CODING_AGENT_DIR".into(),
        env.path("agent").display().to_string(),
    );

    configuration(&context)
        .add(&sample())
        .expect("register sample");

    let document = document(&env, "agent/mcp-adapter.json");
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
    let env = TestEnv::new();
    let project = r#"{"mcpServers":{"sample":{"url":"https://project","disabled":true}}}"#;
    env.write_file(".pi/mcp-adapter.json", project);
    let mut configuration = configuration(&context(&env));

    configuration.add(&sample()).expect("existing registration");

    assert_eq!(env.read_file(".pi/mcp-adapter.json"), project);
    env.assert_not_exists(".pi/agent/mcp-adapter.json");
    let (inventory, definitions) = configuration.inventory();
    assert_eq!(inventory["sample"], env.path(".pi/mcp-adapter.json"));
    assert_eq!(definitions["sample"]["disabled"], true);
}

#[test]
fn removing_user_entry_reports_remaining_highest_layer() {
    let env = TestEnv::new();
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
    let mut configuration = configuration(&context(&env));

    let removal = configuration.remove("sample").expect("remove user entry");

    assert_eq!(
        removal,
        McpRemoval::StillRegistered {
            source: env.path(".pi/mcp-adapter.json")
        }
    );
    let document = document(&env, ".pi/agent/mcp-adapter.json");
    assert!(document["mcpServers"].get("sample").is_none());
    assert_eq!(document["keep"], true);
    assert_eq!(
        configuration.inventory().1["sample"]["url"],
        "https://shared-project"
    );
    assert!(env.read_file(".mcp.json").contains("sample"));
}

#[test]
fn malformed_layer_blocks_loading_without_touching_other_files() {
    let env = TestEnv::new();
    env.write_file(".agents/mcp.json", "{corrupt");
    env.write_file("ace.local.toml", "exclude_mcp = [\"sample\"]\n");

    let error = load_configuration(&context(&env), &[])
        .err()
        .expect("reject malformed layer");

    assert!(error.contains(".agents/mcp.json"), "{error}");
    env.assert_not_exists(".pi/agent/mcp-adapter.json");
    assert_eq!(
        env.read_file("ace.local.toml"),
        "exclude_mcp = [\"sample\"]\n"
    );
    assert_eq!(env.read_file(".agents/mcp.json"), "{corrupt");
}

#[test]
fn explicit_config_replaces_global_adapter_layer_but_not_project_override() {
    let env = TestEnv::new();
    env.write_file(".pi/agent/mcp-adapter.json", "{ignored-corrupt");
    env.write_file(
        "selected.json",
        r#"{"mcpServers":{"sample":{"url":"https://selected"}},"keep":1}"#,
    );
    env.write_file(
        ".mcp.json",
        r#"{"mcpServers":{"sample":{"url":"https://project"}}}"#,
    );
    let mut context = context(&env);
    context.command = vec![
        "wrapper".into(),
        "--mcp-config".into(),
        "selected.json".into(),
    ];

    let removal = configuration(&context)
        .remove("sample")
        .expect("remove selected entry");

    assert_eq!(
        removal,
        McpRemoval::StillRegistered {
            source: env.path(".mcp.json")
        }
    );
    assert_eq!(
        env.read_file(".pi/agent/mcp-adapter.json"),
        "{ignored-corrupt"
    );
    let document = document(&env, "selected.json");
    assert!(document["mcpServers"].get("sample").is_none());
    assert_eq!(document["keep"], 1);
}

fn ancestor_context(env: &TestEnv) -> Invocation {
    env.mkdir("work/project");
    let mut context = context(env);
    context.project_dir = env.path("work/project");
    context
}

#[test]
fn ancestor_discovery_uses_global_boundary_and_nearest_layer() {
    let env = TestEnv::new();
    let context = ancestor_context(&env);
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        &serde_json::json!({"settings":{"ancestorConfigRoots":[env.root()]}}).to_string(),
    );
    env.write_file(".mcp.json", r#"{"mcpServers":{"sample":{"url":"https://outer","headers":{"Keep":"outer"}},"outer":{"url":"https://outer"}}}"#);
    env.write_file(
        "work/.pi/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://near"}}}"#,
    );
    let mut configuration = configuration(&context);

    let removal = configuration
        .remove("sample")
        .expect("retain ancestor registration");

    assert_eq!(
        removal,
        McpRemoval::StillRegistered {
            source: env.path("work/.pi/mcp-adapter.json")
        }
    );
    let (inventory, definitions) = configuration.inventory();
    assert!(inventory.contains_key("outer"));
    assert_eq!(definitions["sample"]["url"], "https://near");
    assert_eq!(definitions["sample"]["headers"]["Keep"], "outer");
}

#[test]
fn project_cannot_enable_ancestor_discovery() {
    let env = TestEnv::new();
    let context = ancestor_context(&env);
    env.write_file(
        "work/project/.pi/mcp-adapter.json",
        &serde_json::json!({"settings":{"ancestorConfigRoots":[env.root()]}}).to_string(),
    );
    env.write_file(
        "work/.mcp.json",
        r#"{"mcpServers":{"ancestor":{"url":"https://outside"}}}"#,
    );

    assert!(
        !configuration(&context)
            .inventory()
            .0
            .contains_key("ancestor")
    );
}

#[test]
fn nearer_ancestor_symlink_retains_its_precedence_and_provenance() {
    let env = TestEnv::new();
    let context = ancestor_context(&env);
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        &serde_json::json!({"settings":{"ancestorConfigRoots":[env.root()]}}).to_string(),
    );
    env.write_file(
        ".mcp.json",
        r#"{"mcpServers":{"sample":{"url":"https://ancestor"}}}"#,
    );
    env.symlink(".mcp.json", "work/.mcp.json");

    let removal = configuration(&context)
        .remove("sample")
        .expect("report nearest alias");

    assert_eq!(
        removal,
        McpRemoval::StillRegistered {
            source: env.path("work/.mcp.json")
        }
    );
}

#[test]
fn user_config_symlink_is_preserved() {
    let env = TestEnv::new();
    env.write_file("owned/adapter.json", r#"{"mcpServers":{},"keep":true}"#);
    env.mkdir(".pi/agent");
    env.symlink("owned/adapter.json", ".pi/agent/mcp-adapter.json");

    configuration(&context(&env))
        .add(&sample())
        .expect("register through symlink");

    env.assert_symlink(".pi/agent/mcp-adapter.json", "owned/adapter.json");
    assert_eq!(
        document(&env, "owned/adapter.json")["mcpServers"]["sample"]["url"],
        "https://example.com/mcp"
    );
    assert_eq!(document(&env, "owned/adapter.json")["keep"], true);
}

#[test]
fn selected_user_file_alias_does_not_leave_a_stale_project_registration() {
    let env = TestEnv::new();
    env.write_file(
        ".pi/mcp-adapter.json",
        r#"{"mcpServers":{"sample":{"url":"https://selected"}}}"#,
    );
    env.symlink(".pi/mcp-adapter.json", "selected.json");
    let mut context = context(&env);
    context.command = vec![
        "wrapper".into(),
        "--mcp-config".into(),
        "selected.json".into(),
    ];

    let removal = configuration(&context)
        .remove("sample")
        .expect("remove aliased registration");

    assert_eq!(removal, McpRemoval::Removed);
    assert!(
        document(&env, ".pi/mcp-adapter.json")["mcpServers"]
            .get("sample")
            .is_none()
    );
    env.assert_symlink("selected.json", ".pi/mcp-adapter.json");
}

#[test]
fn unsupported_imports_are_errors_instead_of_empty_inventory() {
    let env = TestEnv::new();
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"imports":["codex"],"mcpServers":{}}"#,
    );

    let error = load_configuration(&context(&env), &[])
        .err()
        .expect("reject unsupported imports");

    assert!(error.contains("cannot inventory imports"), "{error}");
}

#[test]
fn package_defined_servers_do_not_disappear_from_inventory_silently() {
    let env = TestEnv::new();
    env.write_file(
        "adapter/package.json",
        r#"{"name":"pi-mcp-adapter","version":"3.0.0","pi":{"mcp":"mcp.json"}}"#,
    );
    let packages: Vec<PathBuf> = vec![env.path("adapter")];

    let error = load_configuration(&context(&env), &packages)
        .err()
        .expect("reject package MCP");

    assert!(error.contains("package-declared pi.mcp"), "{error}");
}

#[test]
fn concurrent_changes_are_preserved_when_publication_is_rejected() {
    let env = TestEnv::new();
    env.write_file(
        ".pi/agent/mcp-adapter.json",
        r#"{"mcpServers":{},"keep":"original"}"#,
    );
    let mut configuration = configuration(&context(&env));
    let edited = r#"{"mcpServers":{},"keep":"concurrent"}"#;
    env.write_file(".pi/agent/mcp-adapter.json", edited);

    let error = configuration
        .add(&sample())
        .expect_err("reject stale publication");

    assert!(error.contains("changed during MCP registration"), "{error}");
    assert_eq!(env.read_file(".pi/agent/mcp-adapter.json"), edited);
}
