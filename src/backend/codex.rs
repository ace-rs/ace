use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use super::{McpDecl, McpStatus, OneShotRequest, PromptInput, SessionRequest};
use crate::config::ace_toml::Trust;

pub(super) fn is_ready() -> bool {
    std::env::var("CODEX_API_KEY").is_ok()
        || std::env::var("OPENAI_API_KEY").is_ok()
        || home_dir()
            .map(|d| d.join("auth.json").exists())
            .unwrap_or(false)
}

pub(super) fn exec_session(launch: &[String], req: SessionRequest) -> Result<(), std::io::Error> {
    let (program, prefix) = launch.split_first()
        .map(|(p, rest)| (p.as_str(), rest))
        .unwrap_or(("codex", &[][..]));
    let mut cmd = Command::new(program);
    cmd.args(prefix);
    cmd.current_dir(&req.project_dir);

    for (key, val) in &req.env {
        cmd.env(key, val);
    }

    cmd.args(build_session_args(&req));

    Err(crate::platform::exec_replace(cmd))
}

pub(super) fn exec_one_shot(launch: &[String], req: OneShotRequest) -> Result<Output, std::io::Error> {
    let (program, prefix) = launch.split_first()
        .map(|(p, rest)| (p.as_str(), rest))
        .unwrap_or(("codex", &[][..]));
    let mut cmd = Command::new(program);
    cmd.args(prefix);
    cmd.current_dir(&req.project_dir);

    for (key, val) in &req.env {
        cmd.env(key, val);
    }

    cmd.args(build_one_shot_args(&req));

    if matches!(req.prompt, PromptInput::Stdin) {
        cmd.stdin(Stdio::inherit());
    }

    cmd.output()
}

/// Translate `SessionRequest` into codex's CLI argv (post-binary). Pure function.
fn build_session_args(req: &SessionRequest) -> Vec<String> {
    let mut args = Vec::new();

    if req.resume {
        args.extend(["resume", "--last"].map(String::from));
    }

    args.extend(trust_args(req.trust).iter().map(|s| (*s).to_string()));

    if !req.resume {
        args.push("-c".to_string());
        args.push(format!(
            "developer_instructions={}",
            toml::Value::String(req.session_prompt.clone()),
        ));
    }

    args.extend(req.extra_args.iter().cloned());
    args
}

/// Translate `OneShotRequest` into codex's `exec` argv. Inline prompts are
/// passed as the positional prompt argument; Stdin uses codex's `-` sentinel
/// (verified against codex-rs/exec/src/cli.rs `StdinPromptBehavior::Forced`).
fn build_one_shot_args(req: &OneShotRequest) -> Vec<String> {
    let mut args = vec!["exec".to_string()];
    args.extend(req.extra_args.iter().cloned());
    match &req.prompt {
        PromptInput::Inline(text) => args.push(text.clone()),
        PromptInput::Stdin => args.push("-".to_string()),
    }
    args
}

/// CLI flags for the given trust level.
///
/// `Auto` mirrors `--full-auto` (`-a on-request --sandbox workspace-write`) except
/// the sandbox is raised to `danger-full-access`: ACE typically runs inside an
/// externally-sandboxed environment, so codex's internal sandbox fights the outer
/// one instead of adding protection.
fn trust_args(trust: Trust) -> &'static [&'static str] {
    match trust {
        Trust::Auto => &[
            "--ask-for-approval", "on-request",
            "--sandbox", "danger-full-access",
        ],
        Trust::Yolo => &["--dangerously-bypass-approvals-and-sandbox"],
        Trust::Default => &[],
    }
}

pub(super) fn mcp_list(_project_dir: &std::path::Path) -> HashSet<String> {
    // Best-effort: create home dir so CLI commands work.
    let _ = ensure_home_dir();

    let output = Command::new("codex")
        .args(["mcp", "list", "--json"])
        .output();

    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            parse_list_output(&stdout)
        }
        _ => list_from_config(),
    }
}

pub(super) fn mcp_add(entry: &McpDecl, _project_dir: &std::path::Path) -> Result<(), String> {
    ensure_home_dir()?;

    if let Some(args) = build_mcp_add_args(entry) {
        let output = Command::new("codex")
            .args(&args)
            .output()
            .map_err(|e| format!("codex: {e}"))?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    add_to_config(entry)
}

pub(super) fn mcp_remove(name: &str, _project_dir: &std::path::Path) -> Result<(), String> {
    ensure_home_dir()?;

    let args = build_mcp_remove_args(name);
    let output = Command::new("codex")
        .args(&args)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(_) => return remove_from_config(name),
    };

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(stderr.trim().to_string())
}

pub(super) fn mcp_check(names: &[String], _project_dir: &std::path::Path) -> Result<Vec<McpStatus>, String> {
    ensure_home_dir()?;

    let prompt = build_check_prompt(names);

    let schema = tempfile::NamedTempFile::new()
        .map_err(|e| format!("schema temp file: {e}"))?;
    std::fs::write(schema.path(), CHECK_SCHEMA)
        .map_err(|e| format!("write schema: {e}"))?;

    let output_file = tempfile::NamedTempFile::new()
        .map_err(|e| format!("output temp file: {e}"))?;

    let output = Command::new("codex")
        .args([
            "exec",
            "-o",
            output_file.path().to_string_lossy().as_ref(),
            "--output-schema",
            schema.path().to_string_lossy().as_ref(),
            &prompt,
        ])
        .output()
        .map_err(|e| format!("codex: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("codex: {}", stderr.trim()));
    }

    let content = std::fs::read_to_string(output_file.path())
        .map_err(|e| format!("read output file: {e}"))?;

    Ok(parse_check_output(&content))
}

const CHECK_SCHEMA: &str = r#"{"type":"object","properties":{"statuses":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"ok":{"type":"boolean"}},"required":["name","ok"],"additionalProperties":false}}},"required":["statuses"],"additionalProperties":false}"#;

/// Returns Codex's home directory (`$CODEX_HOME` or `~/.codex`).
fn home_dir() -> Option<PathBuf> {
    std::env::var("CODEX_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| crate::paths::home_dir().map(|h| h.join(".codex")))
}

fn config_path() -> Option<PathBuf> {
    home_dir().map(|d| d.join("config.toml"))
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    std::fs::create_dir_all(path)
        .map_err(|e| format!("create {}: {e}", path.display()))
}

fn ensure_home_dir() -> Result<PathBuf, String> {
    let home = home_dir().ok_or("cannot resolve Codex home".to_string())?;
    ensure_dir(&home)?;
    Ok(home)
}

fn list_from_config() -> HashSet<String> {
    let Some(path) = config_path() else {
        return HashSet::new();
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashSet::new(),
    };
    parse_mcp_names(&content)
}

fn add_to_config(entry: &McpDecl) -> Result<(), String> {
    use std::io::Write;

    let Some(path) = config_path() else {
        return Err("cannot resolve Codex config path".to_string());
    };

    let existing = if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?
    } else {
        String::new()
    };

    let output = merge_mcp_entry(&existing, entry)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {}: {e}", parent.display()))?;
    }

    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    file.write_all(output.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))?;

    Ok(())
}

fn build_mcp_add_args(entry: &McpDecl) -> Option<Vec<String>> {
    if !entry.headers.is_empty() {
        return None;
    }

    Some(vec![
        "mcp".to_string(),
        "add".to_string(),
        entry.name.clone(),
        "--url".to_string(),
        entry.url.clone(),
    ])
}

fn build_mcp_remove_args(name: &str) -> Vec<String> {
    vec![
        "mcp".to_string(),
        "remove".to_string(),
        name.to_string(),
    ]
}

fn build_check_prompt(names: &[String]) -> String {
    format!(
        "You have MCP servers registered. For each of the following, call any tool to verify \
         it responds. Reply with only a JSON object matching this shape: \
         {{\"statuses\":[{{\"name\":\"...\",\"ok\":true/false}}]}}. \
         Servers: {}",
        names.join(", ")
    )
}

fn parse_list_output(output: &str) -> HashSet<String> {
    let parsed: serde_json::Value = match serde_json::from_str(output) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };

    let Some(entries) = parsed.as_array() else {
        return HashSet::new();
    };

    entries
        .iter()
        .filter_map(|entry| entry.get("name").and_then(|v| v.as_str()))
        .map(ToString::to_string)
        .collect()
}

fn remove_from_config(name: &str) -> Result<(), String> {
    use std::io::Write;

    let Some(path) = config_path() else {
        return Ok(());
    };

    let existing = if path.exists() {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?
    } else {
        return Ok(());
    };

    let output = remove_mcp_entry(&existing, name)?;

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create {}: {e}", parent.display()))?;
    }

    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    file.write_all(output.as_bytes())
        .map_err(|e| format!("write {}: {e}", path.display()))?;

    Ok(())
}

fn parse_mcp_names(toml_text: &str) -> HashSet<String> {
    let parsed: toml::Value = match toml::from_str(toml_text) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };

    let Some(servers) = parsed.get("mcp_servers").and_then(|v| v.as_table()) else {
        return HashSet::new();
    };

    servers
        .iter()
        .filter(|(_, cfg)| cfg
            .get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(true))
        .map(|(name, _)| name.clone())
        .collect()
}

fn merge_mcp_entry(existing_toml: &str, entry: &McpDecl) -> Result<String, String> {
    let mut root = parse_or_empty_table(existing_toml)?;

    let root_table = root
        .as_table_mut()
        .ok_or("config root is not a table")?;

    let mcp_servers = root_table
        .entry("mcp_servers".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .ok_or("mcp_servers is not a table")?;

    let mut server = toml::map::Map::new();
    server.insert("url".to_string(), toml::Value::String(entry.url.clone()));

    if !entry.headers.is_empty() {
        let mut headers = toml::map::Map::new();
        let mut sorted_headers: Vec<(&String, &String)> = entry.headers.iter().collect();
        sorted_headers.sort_by_key(|(k, _)| k.as_str());
        for (key, value) in sorted_headers {
            headers.insert(key.clone(), toml::Value::String(value.clone()));
        }
        server.insert("http_headers".to_string(), toml::Value::Table(headers));
    }

    mcp_servers.insert(entry.name.clone(), toml::Value::Table(server));

    toml::to_string_pretty(&root).map_err(|e| format!("serialize config: {e}"))
}

fn remove_mcp_entry(existing_toml: &str, name: &str) -> Result<String, String> {
    let mut root = parse_or_empty_table(existing_toml)?;

    if let Some(mcp_servers) = root.get_mut("mcp_servers").and_then(|v| v.as_table_mut()) {
        mcp_servers.remove(name);
    }

    toml::to_string_pretty(&root).map_err(|e| format!("serialize config: {e}"))
}

fn parse_or_empty_table(existing_toml: &str) -> Result<toml::Value, String> {
    if existing_toml.trim().is_empty() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }

    let parsed: toml::Value =
        toml::from_str(existing_toml).map_err(|e| format!("parse config: {e}"))?;

    if !parsed.is_table() {
        return Err("config root is not a table".to_string());
    }

    Ok(parsed)
}

fn parse_check_output(output: &str) -> Vec<McpStatus> {
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(output)
        && let Some(statuses) = parsed.get("statuses")
    {
        let result = super::parse_status_array(&statuses.to_string());
        if !result.is_empty() {
            return result;
        }
    }

    let result = super::parse_status_array(output);
    if !result.is_empty() {
        return result;
    }

    if let Some(statuses_pos) = output.find("\"statuses\"")
        && let Some(start) = output[statuses_pos..].find('[')
    {
        let start = statuses_pos + start;
        if let Some(end) = output[start..].find(']') {
            return super::parse_status_array(&output[start..=start + end]);
        }
    }

    if let Some(start) = output.find('[')
        && let Some(end) = output.rfind(']')
    {
        return super::parse_status_array(&output[start..=end]);
    }

    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn req() -> SessionRequest {
        SessionRequest {
            trust: Trust::Default,
            session_prompt: "SP".to_string(),
            project_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            extra_args: Vec::new(),
            resume: false,
        }
    }

    fn one_shot(prompt: PromptInput) -> OneShotRequest {
        OneShotRequest {
            prompt,
            project_dir: PathBuf::from("/tmp"),
            env: HashMap::new(),
            extra_args: Vec::new(),
        }
    }

    #[test]
    fn session_args_default_includes_developer_instructions() {
        let args = build_session_args(&req());
        assert!(args.iter().any(|a| a == "-c"));
        assert!(args.iter().any(|a| a.starts_with("developer_instructions=")));
    }

    #[test]
    fn one_shot_args_inline_uses_exec_subcommand() {
        let args = build_one_shot_args(&one_shot(PromptInput::Inline("hello".into())));
        assert_eq!(args.first().map(String::as_str), Some("exec"));
        assert_eq!(args.last().map(String::as_str), Some("hello"));
        assert!(!args.iter().any(|a| a.starts_with("developer_instructions=")),
            "one-shot must not carry session-only flags");
    }

    #[test]
    fn one_shot_args_stdin_uses_dash_sentinel() {
        let args = build_one_shot_args(&one_shot(PromptInput::Stdin));
        assert_eq!(args, vec!["exec", "-"]);
    }

    #[test]
    fn one_shot_args_extra_args_precede_prompt() {
        let mut r = one_shot(PromptInput::Inline("hi".into()));
        r.extra_args = vec!["--model".to_string(), "gpt-5".to_string()];
        let args = build_one_shot_args(&r);
        assert_eq!(args, vec!["exec", "--model", "gpt-5", "hi"]);
    }

    #[test]
    fn trust_default_passes_no_flags() {
        assert!(trust_args(Trust::Default).is_empty());
    }

    #[test]
    fn trust_auto_bypasses_sandbox_with_on_request_approval() {
        assert_eq!(
            trust_args(Trust::Auto),
            &["--ask-for-approval", "on-request", "--sandbox", "danger-full-access"],
        );
    }

    #[test]
    fn trust_yolo_bypasses_approvals_and_sandbox() {
        assert_eq!(
            trust_args(Trust::Yolo),
            &["--dangerously-bypass-approvals-and-sandbox"],
        );
    }

    #[test]
    fn parse_mcp_names_extracts_declared_servers() {
        let toml = r#"
[model_providers.openai]
name = "OpenAI"
base_url = "https://api.openai.com/v1"

[mcp_servers.linear]
url = "https://mcp.linear.app/mcp"

[mcp_servers.github]
url = "https://api.githubcopilot.com/mcp/"
"#;

        let names = parse_mcp_names(toml);
        assert_eq!(names.len(), 2);
        assert!(names.contains("linear"));
        assert!(names.contains("github"));
    }

    #[test]
    fn parse_mcp_names_ignores_missing_section() {
        let names = parse_mcp_names("model = \"gpt-5\"");
        assert!(names.is_empty());
    }

    #[test]
    fn parse_mcp_names_invalid_toml_returns_empty() {
        let names = parse_mcp_names("not valid = = toml");
        assert!(names.is_empty());
    }

    #[test]
    fn build_mcp_add_args_without_headers_uses_cli() {
        let entry = McpDecl {
            name: "linear".to_string(),
            url: "https://mcp.linear.app/mcp".to_string(),
            headers: std::collections::HashMap::new(),
            instructions: String::new(),
        };

        let args = build_mcp_add_args(&entry).expect("should use CLI");
        assert_eq!(
            args,
            vec!["mcp", "add", "linear", "--url", "https://mcp.linear.app/mcp"]
        );
    }

    #[test]
    fn build_mcp_add_args_with_headers_requires_fallback() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer tok".to_string());

        let entry = McpDecl {
            name: "github".to_string(),
            url: "https://api.githubcopilot.com/mcp/".to_string(),
            headers,
            instructions: String::new(),
        };

        assert!(build_mcp_add_args(&entry).is_none());
    }

    #[test]
    fn build_mcp_remove_args_basic() {
        let args = build_mcp_remove_args("linear");
        assert_eq!(args, vec!["mcp", "remove", "linear"]);
    }

    #[test]
    fn parse_list_output_extracts_names() {
        let output = r#"[
  {
    "name": "linear",
    "enabled": true,
    "transport": {"type": "streamable_http", "url": "https://mcp.linear.app/mcp"}
  },
  {
    "name": "github",
    "enabled": false,
    "transport": {"type": "streamable_http", "url": "https://api.githubcopilot.com/mcp/"}
  }
]"#;

        let names = parse_list_output(output);
        assert_eq!(names.len(), 2);
        assert!(names.contains("linear"));
        assert!(names.contains("github"));
    }

    #[test]
    fn parse_list_output_invalid_returns_empty() {
        assert!(parse_list_output("not json").is_empty());
        assert!(parse_list_output("{}").is_empty());
    }

    #[test]
    fn merge_mcp_entry_basic() {
        let entry = McpDecl {
            name: "linear".to_string(),
            url: "https://mcp.linear.app/mcp".to_string(),
            headers: std::collections::HashMap::new(),
            instructions: String::new(),
        };

        let output = merge_mcp_entry("", &entry).expect("should merge");
        let parsed: toml::Value = toml::from_str(&output).expect("valid toml");
        assert_eq!(parsed["mcp_servers"]["linear"]["url"].as_str(), Some("https://mcp.linear.app/mcp"));
    }

    #[test]
    fn merge_mcp_entry_with_headers() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer tok".to_string());
        headers.insert("X-Custom".to_string(), "value".to_string());

        let entry = McpDecl {
            name: "github".to_string(),
            url: "https://api.githubcopilot.com/mcp/".to_string(),
            headers,
            instructions: String::new(),
        };

        let output = merge_mcp_entry("", &entry).expect("should merge");
        let parsed: toml::Value = toml::from_str(&output).expect("valid toml");
        assert_eq!(
            parsed["mcp_servers"]["github"]["http_headers"]["Authorization"].as_str(),
            Some("Bearer tok")
        );
        assert_eq!(
            parsed["mcp_servers"]["github"]["http_headers"]["X-Custom"].as_str(),
            Some("value")
        );
    }

    #[test]
    fn remove_mcp_entry_basic() {
        let existing = r#"
[mcp_servers.linear]
url = "https://mcp.linear.app/mcp"

[mcp_servers.github]
url = "https://api.githubcopilot.com/mcp/"
"#;

        let output = remove_mcp_entry(existing, "linear").expect("should remove");
        let names = parse_mcp_names(&output);
        assert!(!names.contains("linear"));
        assert!(names.contains("github"));
    }

    #[test]
    fn parse_check_output_valid_json_array() {
        let output = r#"[{"name":"linear","ok":true},{"name":"github","ok":false}]"#;
        let statuses = parse_check_output(output);
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].name, "linear");
        assert!(statuses[0].ok);
        assert_eq!(statuses[1].name, "github");
        assert!(!statuses[1].ok);
    }

    #[test]
    fn parse_check_output_valid_json_object() {
        let output = r#"{"statuses":[{"name":"linear","ok":true},{"name":"github","ok":false}]}"#;
        let statuses = parse_check_output(output);
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].name, "linear");
        assert!(statuses[0].ok);
        assert_eq!(statuses[1].name, "github");
        assert!(!statuses[1].ok);
    }

    #[test]
    fn parse_check_output_extracts_embedded_array() {
        let output = r#"Health check complete: [{"name":"linear","ok":true}]"#;
        let statuses = parse_check_output(output);
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "linear");
        assert!(statuses[0].ok);
    }

    #[test]
    fn parse_check_output_extracts_embedded_statuses_array() {
        let output = r#"Health check complete: {"statuses":[{"name":"linear","ok":true}]}"#;
        let statuses = parse_check_output(output);
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "linear");
        assert!(statuses[0].ok);
    }

    #[test]
    fn parse_check_output_invalid_returns_empty() {
        assert!(parse_check_output("not json").is_empty());
        assert!(parse_check_output("{}").is_empty());
    }

    #[test]
    fn build_mcp_check_prompt_mentions_servers() {
        let names = vec!["linear".to_string(), "github".to_string()];
        let prompt = build_check_prompt(&names);
        assert!(prompt.contains("linear"));
        assert!(prompt.contains("github"));
        assert!(prompt.contains("\"statuses\""));
    }

    #[test]
    fn ensure_dir_creates_nested_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let nested = tmp.path().join("a").join("b").join("c");
        ensure_dir(&nested).expect("should create nested dirs");
        assert!(nested.is_dir(), "nested directory should exist");
    }
}
