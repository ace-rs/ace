use std::collections::HashSet;
use std::path::Path;
use std::process::Output;

use super::{MaterializeError, McpDecl, McpStatus, OneShotOptions, SessionOptions};
use crate::config::ace_toml::Trust;
use crate::session::{Component, Graph, Node, Role};

pub(super) fn is_ready() -> bool {
    let auth = auth_path();
    auth.exists()
        && std::fs::metadata(&auth)
            .map(|m| m.len() > 0)
            .unwrap_or(false)
}

pub(super) fn exec_session(
    launch: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    options: SessionOptions,
) -> Result<(), std::io::Error> {
    write_agent_file(&options.project_dir, &options.session_prompt, model, effort)?;

    let graph = materialize_session_graph(launch, model, effort, &options, None)
        .map_err(std::io::Error::other)?;

    Err(graph.exec_replace())
}

pub(super) fn materialize_session_graph(
    launch: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    options: &SessionOptions,
    endpoint: Option<&crate::session::ControlEndpoint>,
) -> Result<Graph, MaterializeError> {
    let session = build_session_component(launch, model, effort, options);
    if matches!(options.backend_mode, super::BackendMode::Normal) {
        return Ok(Graph::try_new(vec![Node::new(session, Vec::new())])?);
    }
    let endpoint = endpoint.ok_or(MaterializeError::MissingControlEndpoint {
        backend: "opencode",
    })?;
    let Some((hostname, port, endpoint_url)) = endpoint.loopback_http() else {
        return Err(MaterializeError::ControlEndpoint {
            backend: "opencode",
            expected: "loopback HTTP",
        });
    };

    let server = Component::from_launch(
        Role::Server,
        launch,
        "opencode",
        vec![
            "serve".to_string(),
            "--hostname".to_string(),
            hostname.to_string(),
            "--port".to_string(),
            port.to_string(),
        ],
        &options.env,
        &options.project_dir,
    );
    let mut args = Vec::new();
    args.extend([
        "attach".to_string(),
        endpoint_url,
        "--dir".to_string(),
        options.project_dir.to_string_lossy().into_owned(),
    ]);
    if matches!(options.resume, super::ResumeMode::Latest) {
        args.push("--continue".to_string());
    }
    args.extend(options.extra_args.iter().cloned());
    let session = Component::from_launch(
        Role::Session,
        launch,
        "opencode",
        args,
        &options.env,
        &options.project_dir,
    );

    Ok(Graph::try_new(vec![
        Node::new(server, Vec::new()),
        Node::new(session, vec![Role::Server]),
    ])?)
}

fn build_session_component(
    launch: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    options: &SessionOptions,
) -> Component {
    Component::from_launch(
        Role::Session,
        launch,
        "opencode",
        build_session_args(model, effort, options),
        &options.env,
        &options.project_dir,
    )
}

pub(super) fn exec_one_shot(
    launch: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    options: OneShotOptions,
) -> Result<Output, std::io::Error> {
    let (program, prefix) = launch
        .split_first()
        .map(|(p, rest)| (p.as_str(), rest))
        .unwrap_or(("opencode", &[][..]));
    let mut cmd = std::process::Command::new(program);
    cmd.args(prefix);
    cmd.current_dir(&options.project_dir);

    for (key, val) in &options.env {
        cmd.env(key, val);
    }

    cmd.args(build_one_shot_args(model, effort, &options));

    if matches!(options.prompt, super::PromptInput::Stdin) {
        cmd.stdin(std::process::Stdio::inherit());
    }

    cmd.output()
}

pub(super) fn mcp_list(project_dir: &Path) -> HashSet<String> {
    let path = project_dir.join("opencode.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            warn_jsonc(project_dir);
            return HashSet::new();
        }
    };
    parse_mcp_names(&content)
}

pub(super) fn mcp_add(entry: &McpDecl, project_dir: &Path) -> Result<(), String> {
    let path = project_dir.join("opencode.json");
    let existing = if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?
    } else if project_dir.join("opencode.jsonc").exists() {
        return Err(
            "opencode.jsonc found but ACE only supports opencode.json — rename or convert it"
                .to_string(),
        );
    } else {
        String::new()
    };

    let output = merge_mcp_entry(&existing, entry)?;
    std::fs::write(&path, output).map_err(|e| format!("write {}: {e}", path.display()))
}

pub(super) fn mcp_remove(name: &str, project_dir: &Path) -> Result<(), String> {
    let path = project_dir.join("opencode.json");
    if !path.exists() {
        return Ok(());
    }

    let existing =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;

    let output = remove_mcp_entry(&existing, name)?;
    std::fs::write(&path, output).map_err(|e| format!("write {}: {e}", path.display()))
}

/// Best-effort — OpenCode has no structured MCP health check surface.
pub(super) fn mcp_check(_names: &[String], _project_dir: &Path) -> Result<Vec<McpStatus>, String> {
    Ok(Vec::new())
}

// -- internals --

/// Warn if opencode.jsonc exists but opencode.json does not.
fn warn_jsonc(project_dir: &Path) {
    if project_dir.join("opencode.jsonc").exists() {
        eprintln!(
            "warning: opencode.jsonc found but ACE only supports opencode.json — rename or convert it"
        );
    }
}

/// Path to OpenCode's auth file. Respects `OPENCODE_HOME`.
fn auth_path() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("OPENCODE_HOME") {
        return std::path::PathBuf::from(home).join("auth.json");
    }

    // XDG default: ~/.local/share/opencode/auth.json
    if let Some(data) = std::env::var_os("XDG_DATA_HOME") {
        return std::path::PathBuf::from(data).join("opencode/auth.json");
    }

    crate::paths::home_dir()
        .map(|h| h.join(".local/share/opencode/auth.json"))
        .unwrap_or_else(|| std::path::PathBuf::from(".local/share/opencode/auth.json"))
}

/// Write the ACE agent file that carries the session prompt.
fn write_agent_file(
    project_dir: &Path,
    session_prompt: &str,
    model: Option<&str>,
    effort: Option<&str>,
) -> Result<(), std::io::Error> {
    let agents_dir = project_dir.join(".opencode/agents");
    std::fs::create_dir_all(&agents_dir)?;

    let model = model
        .map(|value| format!("model: {}\n", serde_json::Value::String(value.to_string())))
        .unwrap_or_default();
    let variant = effort
        .map(|value| {
            format!(
                "variant: {}\n",
                serde_json::Value::String(value.to_string())
            )
        })
        .unwrap_or_default();
    let content = format!(
        "---\ndescription: \"ACE-provisioned coding session\"\nmode: all\n{model}{variant}---\n\n{session_prompt}\n"
    );
    std::fs::write(agents_dir.join("ace.md"), content)
}

/// OpenCode's interactive mode exposes no approval flags whatsoever —
/// `--dangerously-skip-permissions` belongs to `opencode run`, the one-shot
/// path, which never carries a trust level.
pub(super) fn supports_trust(trust: Trust) -> bool {
    matches!(trust, Trust::Default)
}

/// Translate `SessionOptions` into opencode's interactive argv.
fn build_session_args(
    _model: Option<&str>,
    _effort: Option<&str>,
    options: &SessionOptions,
) -> Vec<String> {
    let mut args = Vec::new();

    if matches!(options.resume, super::ResumeMode::Latest) {
        args.push("--continue".to_string());
    }

    args.extend(["--agent", "ace"].map(String::from));
    args.extend(options.extra_args.iter().cloned());
    args
}

/// Translate `OneShotOptions` into opencode's `run` argv.
fn build_one_shot_args(
    model: Option<&str>,
    effort: Option<&str>,
    options: &OneShotOptions,
) -> Vec<String> {
    let mut args = vec!["run".to_string(), "--agent".to_string(), "ace".to_string()];
    if let Some(value) = model {
        args.extend(["--model".to_string(), value.to_string()]);
    }
    if let Some(value) = effort {
        args.extend(["--variant".to_string(), value.to_string()]);
    }
    args.extend(options.extra_args.iter().cloned());

    match &options.prompt {
        super::PromptInput::Inline(text) => args.push(text.clone()),
        super::PromptInput::Stdin => {} // opencode run reads stdin when no positional prompt
    }
    args
}

fn parse_mcp_names(json: &str) -> HashSet<String> {
    let parsed: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };

    // opencode.json uses "mcpServers" key (same as Claude's .claude.json shape)
    parsed
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

fn merge_mcp_entry(existing_json: &str, entry: &McpDecl) -> Result<String, String> {
    let mut root: serde_json::Value = if existing_json.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(existing_json).map_err(|e| format!("parse opencode.json: {e}"))?
    };

    let servers = root
        .as_object_mut()
        .ok_or("opencode.json root is not an object")?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let mut server = serde_json::Map::new();
    server.insert(
        "url".to_string(),
        serde_json::Value::String(entry.url.clone()),
    );

    if !entry.headers.is_empty() {
        let mut headers = serde_json::Map::new();
        let mut sorted: Vec<(&String, &String)> = entry.headers.iter().collect();
        sorted.sort_by_key(|(k, _)| k.as_str());
        for (key, value) in sorted {
            headers.insert(key.clone(), serde_json::Value::String(value.clone()));
        }
        server.insert("headers".to_string(), serde_json::Value::Object(headers));
    }

    servers
        .as_object_mut()
        .ok_or("mcpServers is not an object")?
        .insert(entry.name.clone(), serde_json::Value::Object(server));

    serde_json::to_string_pretty(&root).map_err(|e| format!("serialize opencode.json: {e}"))
}

fn remove_mcp_entry(existing_json: &str, name: &str) -> Result<String, String> {
    let mut root: serde_json::Value =
        serde_json::from_str(existing_json).map_err(|e| format!("parse opencode.json: {e}"))?;

    if let Some(servers) = root.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove(name);
    }

    serde_json::to_string_pretty(&root).map_err(|e| format!("serialize opencode.json: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn session_options() -> SessionOptions {
        SessionOptions {
            trust: crate::config::ace_toml::Trust::Default,
            session_prompt: "SP".to_string(),
            project_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            extra_args: Vec::new(),
            resume: super::super::ResumeMode::Fresh,
            backend_mode: super::super::BackendMode::Normal,
        }
    }

    fn one_shot(prompt: super::super::PromptInput) -> OneShotOptions {
        OneShotOptions {
            prompt,
            project_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            extra_args: Vec::new(),
        }
    }

    // -- session args --

    #[test]
    fn session_args_default() {
        let args = build_session_args(None, None, &session_options());
        assert_eq!(args, vec!["--agent", "ace"]);
    }

    #[test]
    fn session_component_carries_launch_context() {
        let mut options = session_options();
        options.env.insert("TOKEN".into(), "secret".into());
        let launch = ["wrapper".to_string(), "opencode".to_string()];

        let component = build_session_component(&launch, None, None, &options);

        assert_eq!(component.role(), crate::session::Role::Session);
        assert_eq!(component.program(), "wrapper");
        assert_eq!(component.args(), ["opencode", "--agent", "ace"]);
        assert_eq!(component.working_dir(), Path::new("/tmp"));
        assert_eq!(
            component.env().get("TOKEN").map(String::as_str),
            Some("secret")
        );
    }

    #[test]
    fn server_backed_graph_uses_serve_and_attach() {
        let mut options = session_options();
        options.resume = super::super::ResumeMode::Latest;
        options.env.insert("TOKEN".into(), "secret".into());
        options.extra_args.push("--mini".to_string());
        let port = std::num::NonZeroU16::new(
            u16::try_from(std::process::id() % u32::from(u16::MAX - 1) + 1)
                .expect("bounded test port"),
        )
        .expect("non-zero test port");
        options.backend_mode = super::super::BackendMode::WithServer;
        let endpoint = crate::session::ControlEndpoint::LoopbackHttp(port);
        let launch = ["wrapper".to_string(), "opencode".to_string()];

        let graph = materialize_session_graph(&launch, None, None, &options, Some(&endpoint))
            .expect("valid graph");
        let server = &graph.nodes()[0];
        let session = &graph.nodes()[1];
        let port = port.get().to_string();
        let url = format!("http://127.0.0.1:{port}");

        assert_eq!(server.component().role(), crate::session::Role::Server);
        assert_eq!(
            server.component().args(),
            [
                "opencode",
                "serve",
                "--hostname",
                "127.0.0.1",
                "--port",
                &port
            ]
        );
        assert_eq!(session.dependencies(), [crate::session::Role::Server]);
        assert_eq!(session.component().role(), crate::session::Role::Session);
        assert_eq!(
            &session.component().args()[..5],
            ["opencode", "attach", &url, "--dir", "/tmp"]
        );
        assert_eq!(&session.component().args()[5..], ["--continue", "--mini"]);
        for node in graph.nodes() {
            assert_eq!(node.component().working_dir(), Path::new("/tmp"));
            assert_eq!(
                node.component().env().get("TOKEN").map(String::as_str),
                Some("secret")
            );
        }
    }

    #[test]
    fn server_backed_graph_rejects_non_http_endpoint() {
        let mut options = session_options();
        options.backend_mode = super::super::BackendMode::WithServer;
        let endpoint = crate::session::ControlEndpoint::Unix(PathBuf::from("/tmp/opencode.sock"));

        let error = materialize_session_graph(&[], None, None, &options, Some(&endpoint))
            .expect_err("OpenCode requires a loopback HTTP endpoint");

        assert_eq!(
            error,
            MaterializeError::ControlEndpoint {
                backend: "opencode",
                expected: "loopback HTTP",
            }
        );
    }

    #[test]
    fn session_args_resume() {
        let mut options = session_options();
        options.resume = super::super::ResumeMode::Latest;
        let args = build_session_args(None, None, &options);
        assert_eq!(args, vec!["--continue", "--agent", "ace"]);
    }

    #[test]
    fn session_args_extra_args_come_last() {
        let mut options = session_options();
        options.extra_args = vec!["--model".to_string(), "anthropic/claude-sonnet".to_string()];
        let args = build_session_args(None, None, &options);
        assert_eq!(
            args,
            vec!["--agent", "ace", "--model", "anthropic/claude-sonnet"]
        );
    }

    // -- one-shot args --

    #[test]
    fn one_shot_args_inline() {
        let args = build_one_shot_args(
            None,
            None,
            &one_shot(super::super::PromptInput::Inline("hello".into())),
        );
        assert_eq!(args, vec!["run", "--agent", "ace", "hello"]);
    }

    #[test]
    fn one_shot_args_stdin() {
        let args = build_one_shot_args(None, None, &one_shot(super::super::PromptInput::Stdin));
        assert_eq!(args, vec!["run", "--agent", "ace"]);
    }

    #[test]
    fn one_shot_args_extra_args_before_prompt() {
        let mut r = one_shot(super::super::PromptInput::Inline("hi".into()));
        r.extra_args = vec!["--model".to_string(), "anthropic/claude-sonnet".to_string()];
        let args = build_one_shot_args(None, None, &r);
        assert_eq!(
            args,
            vec![
                "run",
                "--agent",
                "ace",
                "--model",
                "anthropic/claude-sonnet",
                "hi"
            ]
        );
    }

    #[test]
    fn session_model_and_effort_configure_the_ace_agent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_dir = tmp.path().canonicalize().expect("canonicalize");

        write_agent_file(
            &project_dir,
            "test prompt",
            Some("provider: model"),
            Some("yes"),
        )
        .expect("should write");

        let content = std::fs::read_to_string(project_dir.join(".opencode/agents/ace.md"))
            .expect("should read");
        assert!(content.contains("model: \"provider: model\"\n"));
        assert!(content.contains("variant: \"yes\"\n"));
    }

    #[test]
    fn run_model_and_variant_precede_passthrough_args() {
        let mut options = one_shot(super::super::PromptInput::Inline("hi".into()));
        options.extra_args = vec!["--model".into(), "override".into()];

        let args = build_one_shot_args(Some("anthropic/claude-sonnet"), Some("max"), &options);

        assert_eq!(
            args,
            vec![
                "run",
                "--agent",
                "ace",
                "--model",
                "anthropic/claude-sonnet",
                "--variant",
                "max",
                "--model",
                "override",
                "hi"
            ]
        );
    }

    // -- parse_mcp_names --

    #[test]
    fn parse_mcp_names_extracts_keys() {
        let json = r#"{
            "mcpServers": {
                "linear": {"url": "https://mcp.linear.app/mcp"},
                "github": {"url": "https://api.githubcopilot.com/mcp/"}
            }
        }"#;
        let names = parse_mcp_names(json);
        assert_eq!(names.len(), 2);
        assert!(names.contains("linear"));
        assert!(names.contains("github"));
    }

    #[test]
    fn parse_mcp_names_missing_field() {
        assert!(parse_mcp_names(r#"{"something": "else"}"#).is_empty());
    }

    #[test]
    fn parse_mcp_names_empty_servers() {
        assert!(parse_mcp_names(r#"{"mcpServers": {}}"#).is_empty());
    }

    #[test]
    fn parse_mcp_names_invalid_json() {
        assert!(parse_mcp_names("not json").is_empty());
    }

    // -- merge_mcp_entry --

    #[test]
    fn merge_into_empty() {
        let entry = McpDecl {
            name: "linear".to_string(),
            url: "https://mcp.linear.app/mcp".to_string(),
            headers: HashMap::new(),
            instructions: String::new(),
        };

        let output = merge_mcp_entry("", &entry).expect("should merge");
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(
            parsed["mcpServers"]["linear"]["url"].as_str(),
            Some("https://mcp.linear.app/mcp")
        );
    }

    #[test]
    fn merge_preserves_existing() {
        let existing =
            r#"{"mcpServers":{"github":{"url":"https://github.com/mcp"}},"other":"data"}"#;
        let entry = McpDecl {
            name: "linear".to_string(),
            url: "https://mcp.linear.app/mcp".to_string(),
            headers: HashMap::new(),
            instructions: String::new(),
        };

        let output = merge_mcp_entry(existing, &entry).expect("should merge");
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(
            parsed["mcpServers"]["github"]["url"].as_str(),
            Some("https://github.com/mcp")
        );
        assert_eq!(
            parsed["mcpServers"]["linear"]["url"].as_str(),
            Some("https://mcp.linear.app/mcp")
        );
        assert_eq!(parsed["other"].as_str(), Some("data"));
    }

    #[test]
    fn merge_with_headers() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer tok".to_string());

        let entry = McpDecl {
            name: "sentry".to_string(),
            url: "https://mcp.sentry.dev/sse".to_string(),
            headers,
            instructions: String::new(),
        };

        let output = merge_mcp_entry("", &entry).expect("should merge");
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(
            parsed["mcpServers"]["sentry"]["headers"]["Authorization"].as_str(),
            Some("Bearer tok")
        );
    }

    // -- remove_mcp_entry --

    #[test]
    fn remove_existing() {
        let existing = r#"{"mcpServers":{"linear":{"url":"https://mcp.linear.app/mcp"},"github":{"url":"https://github.com/mcp"}}}"#;
        let output = remove_mcp_entry(existing, "linear").expect("should remove");
        let names = parse_mcp_names(&output);
        assert!(!names.contains("linear"));
        assert!(names.contains("github"));
    }

    #[test]
    fn remove_nonexistent_is_ok() {
        let existing = r#"{"mcpServers":{"linear":{"url":"https://mcp.linear.app/mcp"}}}"#;
        let output = remove_mcp_entry(existing, "unknown").expect("should succeed");
        let names = parse_mcp_names(&output);
        assert!(names.contains("linear"));
    }

    // -- mcp_add jsonc guard --

    #[test]
    fn mcp_add_rejects_jsonc_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_dir = tmp.path().canonicalize().expect("canonicalize");
        std::fs::write(project_dir.join("opencode.jsonc"), "{}").expect("write");

        let entry = McpDecl {
            name: "linear".to_string(),
            url: "https://mcp.linear.app/mcp".to_string(),
            headers: HashMap::new(),
            instructions: String::new(),
        };

        let err = mcp_add(&entry, &project_dir).expect_err("should reject jsonc");
        assert!(err.contains("opencode.jsonc"), "error should mention jsonc");
    }

    #[test]
    fn mcp_list_returns_empty_for_jsonc_only() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_dir = tmp.path().canonicalize().expect("canonicalize");
        std::fs::write(
            project_dir.join("opencode.jsonc"),
            r#"{"mcpServers":{"linear":{"url":"x"}}}"#,
        )
        .expect("write");

        let names = mcp_list(&project_dir);
        assert!(
            names.is_empty(),
            "should return empty when only jsonc exists"
        );
    }

    // -- write_agent_file --

    #[test]
    fn agent_file_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_dir = tmp.path().canonicalize().expect("canonicalize");
        write_agent_file(&project_dir, "test prompt", None, None).expect("should write");

        let path = project_dir.join(".opencode/agents/ace.md");
        assert!(path.exists(), "agent file should exist");

        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("mode: all"));
        assert!(content.contains("test prompt"));
    }
}
