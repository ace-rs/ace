use std::collections::HashSet;
use std::path::Path;
use std::process::Output;

use super::{McpDecl, McpStatus, OneShotRequest, SessionRequest};

pub(super) fn is_ready() -> bool {
    let auth = auth_path();
    auth.exists() && std::fs::metadata(&auth).map(|m| m.len() > 0).unwrap_or(false)
}

pub(super) fn exec_session(launch: &[String], req: SessionRequest) -> Result<(), std::io::Error> {
    write_agent_file(&req.project_dir, &req.session_prompt)?;

    let (program, prefix) = launch
        .split_first()
        .map(|(p, rest)| (p.as_str(), rest))
        .unwrap_or(("opencode", &[][..]));
    let mut cmd = std::process::Command::new(program);
    cmd.args(prefix);
    cmd.current_dir(&req.project_dir);

    for (key, val) in &req.env {
        cmd.env(key, val);
    }

    cmd.args(build_session_args(&req));

    Err(crate::platform::exec_replace(cmd))
}

pub(super) fn exec_one_shot(launch: &[String], req: OneShotRequest) -> Result<Output, std::io::Error> {
    let (program, prefix) = launch
        .split_first()
        .map(|(p, rest)| (p.as_str(), rest))
        .unwrap_or(("opencode", &[][..]));
    let mut cmd = std::process::Command::new(program);
    cmd.args(prefix);
    cmd.current_dir(&req.project_dir);

    for (key, val) in &req.env {
        cmd.env(key, val);
    }

    cmd.args(build_one_shot_args(&req));

    if matches!(req.prompt, super::PromptInput::Stdin) {
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
        std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?
    } else if project_dir.join("opencode.jsonc").exists() {
        return Err("opencode.jsonc found but ACE only supports opencode.json — rename or convert it".to_string());
    } else {
        String::new()
    };

    let output = merge_mcp_entry(&existing, entry)?;
    std::fs::write(&path, output)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

pub(super) fn mcp_remove(name: &str, project_dir: &Path) -> Result<(), String> {
    let path = project_dir.join("opencode.json");
    if !path.exists() {
        return Ok(());
    }

    let existing = std::fs::read_to_string(&path)
        .map_err(|e| format!("read {}: {e}", path.display()))?;

    let output = remove_mcp_entry(&existing, name)?;
    std::fs::write(&path, output)
        .map_err(|e| format!("write {}: {e}", path.display()))
}

/// Best-effort — OpenCode has no structured MCP health check surface.
pub(super) fn mcp_check(_names: &[String], _project_dir: &Path) -> Result<Vec<McpStatus>, String> {
    Ok(Vec::new())
}

// -- internals --

/// Warn if opencode.jsonc exists but opencode.json does not.
fn warn_jsonc(project_dir: &Path) {
    if project_dir.join("opencode.jsonc").exists() {
        eprintln!("warning: opencode.jsonc found but ACE only supports opencode.json — rename or convert it");
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
fn write_agent_file(project_dir: &Path, session_prompt: &str) -> Result<(), std::io::Error> {
    let agents_dir = project_dir.join(".opencode/agents");
    std::fs::create_dir_all(&agents_dir)?;

    let content = format!(
        "---\ndescription: \"ACE-provisioned coding session\"\nmode: all\n---\n\n{session_prompt}\n"
    );
    std::fs::write(agents_dir.join("ace.md"), content)
}

/// Translate `SessionRequest` into opencode's interactive argv.
fn build_session_args(req: &SessionRequest) -> Vec<String> {
    let mut args = Vec::new();

    if req.resume {
        args.push("--continue".to_string());
    }

    // OpenCode interactive mode has no trust flags.
    // `--dangerously-skip-permissions` is `opencode run` only (one-shot).
    match req.trust {
        crate::config::ace_toml::Trust::Default => {}
        crate::config::ace_toml::Trust::Auto | crate::config::ace_toml::Trust::Yolo => {}
    }

    args.extend(["--agent", "ace"].map(String::from));
    args.extend(req.extra_args.iter().cloned());
    args
}

/// Translate `OneShotRequest` into opencode's `run` argv.
fn build_one_shot_args(req: &OneShotRequest) -> Vec<String> {
    let mut args = vec!["run".to_string(), "--agent".to_string(), "ace".to_string()];
    args.extend(req.extra_args.iter().cloned());

    match &req.prompt {
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
        serde_json::from_str(existing_json)
            .map_err(|e| format!("parse opencode.json: {e}"))?
    };

    let servers = root
        .as_object_mut()
        .ok_or("opencode.json root is not an object")?
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    let mut server = serde_json::Map::new();
    server.insert("url".to_string(), serde_json::Value::String(entry.url.clone()));

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

    serde_json::to_string_pretty(&root)
        .map_err(|e| format!("serialize opencode.json: {e}"))
}

fn remove_mcp_entry(existing_json: &str, name: &str) -> Result<String, String> {
    let mut root: serde_json::Value = serde_json::from_str(existing_json)
        .map_err(|e| format!("parse opencode.json: {e}"))?;

    if let Some(servers) = root.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        servers.remove(name);
    }

    serde_json::to_string_pretty(&root)
        .map_err(|e| format!("serialize opencode.json: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn req() -> SessionRequest {
        SessionRequest {
            trust: crate::config::ace_toml::Trust::Default,
            session_prompt: "SP".to_string(),
            project_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            extra_args: Vec::new(),
            resume: false,
        }
    }

    fn one_shot(prompt: super::super::PromptInput) -> OneShotRequest {
        OneShotRequest {
            prompt,
            project_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            extra_args: Vec::new(),
        }
    }

    // -- session args --

    #[test]
    fn session_args_default() {
        let args = build_session_args(&req());
        assert_eq!(args, vec!["--agent", "ace"]);
    }

    #[test]
    fn session_args_resume() {
        let mut r = req();
        r.resume = true;
        let args = build_session_args(&r);
        assert_eq!(args, vec!["--continue", "--agent", "ace"]);
    }

    #[test]
    fn session_args_extra_args_come_last() {
        let mut r = req();
        r.extra_args = vec!["--model".to_string(), "anthropic/claude-sonnet".to_string()];
        let args = build_session_args(&r);
        assert_eq!(args, vec!["--agent", "ace", "--model", "anthropic/claude-sonnet"]);
    }

    // -- one-shot args --

    #[test]
    fn one_shot_args_inline() {
        let args = build_one_shot_args(&one_shot(super::super::PromptInput::Inline("hello".into())));
        assert_eq!(args, vec!["run", "--agent", "ace", "hello"]);
    }

    #[test]
    fn one_shot_args_stdin() {
        let args = build_one_shot_args(&one_shot(super::super::PromptInput::Stdin));
        assert_eq!(args, vec!["run", "--agent", "ace"]);
    }

    #[test]
    fn one_shot_args_extra_args_before_prompt() {
        let mut r = one_shot(super::super::PromptInput::Inline("hi".into()));
        r.extra_args = vec!["--model".to_string(), "anthropic/claude-sonnet".to_string()];
        let args = build_one_shot_args(&r);
        assert_eq!(args, vec!["run", "--agent", "ace", "--model", "anthropic/claude-sonnet", "hi"]);
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
        let existing = r#"{"mcpServers":{"github":{"url":"https://github.com/mcp"}},"other":"data"}"#;
        let entry = McpDecl {
            name: "linear".to_string(),
            url: "https://mcp.linear.app/mcp".to_string(),
            headers: HashMap::new(),
            instructions: String::new(),
        };

        let output = merge_mcp_entry(existing, &entry).expect("should merge");
        let parsed: serde_json::Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["mcpServers"]["github"]["url"].as_str(), Some("https://github.com/mcp"));
        assert_eq!(parsed["mcpServers"]["linear"]["url"].as_str(), Some("https://mcp.linear.app/mcp"));
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
        std::fs::write(project_dir.join("opencode.jsonc"), r#"{"mcpServers":{"linear":{"url":"x"}}}"#).expect("write");

        let names = mcp_list(&project_dir);
        assert!(names.is_empty(), "should return empty when only jsonc exists");
    }

    // -- write_agent_file --

    #[test]
    fn agent_file_content() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project_dir = tmp.path().canonicalize().expect("canonicalize");
        write_agent_file(&project_dir, "test prompt").expect("should write");

        let path = project_dir.join(".opencode/agents/ace.md");
        assert!(path.exists(), "agent file should exist");

        let content = std::fs::read_to_string(&path).expect("read");
        assert!(content.contains("mode: all"));
        assert!(content.contains("test prompt"));
    }
}
