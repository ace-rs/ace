use std::collections::HashSet;
use std::process::{Command, Output, Stdio};

use super::{McpDecl, McpStatus, OneShotRequest, PromptInput, SessionRequest};
use crate::config::ace_toml::Trust;

pub(super) fn is_ready() -> bool {
    let Some(home) = crate::paths::home_dir() else {
        return false;
    };
    home.join(".claude.json").exists()
}

pub(super) fn exec_session(launch: &[String], req: SessionRequest) -> Result<(), std::io::Error> {
    let (program, prefix) = launch.split_first()
        .map(|(p, rest)| (p.as_str(), rest))
        .unwrap_or(("claude", &[][..]));
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
        .unwrap_or(("claude", &[][..]));
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

/// Translate `SessionRequest` into Claude's CLI argv (post-binary). Pure
/// function — no I/O, no `Command`. Tested below.
fn build_session_args(req: &SessionRequest) -> Vec<String> {
    let mut args = Vec::new();

    if req.resume {
        args.push("--continue".to_string());
    } else {
        args.push("--system-prompt".to_string());
        args.push(req.session_prompt.clone());
    }

    match req.trust {
        Trust::Auto => args.extend(["--permission-mode", "auto"].map(String::from)),
        Trust::Yolo => args.extend(["--permission-mode", "bypassPermissions"].map(String::from)),
        Trust::Default => {}
    }

    args.extend(req.extra_args.iter().cloned());
    args
}

/// Translate `OneShotRequest` into Claude's `-p` argv. Inline prompts pass
/// the text as the `-p` value; Stdin omits the value and the child reads
/// from inherited stdin.
fn build_one_shot_args(req: &OneShotRequest) -> Vec<String> {
    let mut args = vec!["-p".to_string()];
    if let PromptInput::Inline(text) = &req.prompt {
        args.push(text.clone());
    }
    args.extend(req.extra_args.iter().cloned());
    args
}

pub(super) fn mcp_list() -> HashSet<String> {
    let Some(home) = crate::paths::home_dir() else {
        return HashSet::new();
    };

    let path = home.join(".claude.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashSet::new(),
    };

    parse_mcp_names(&content)
}

pub(super) fn mcp_add(entry: &McpDecl) -> Result<(), String> {
    let args = build_mcp_add_args(entry);

    let output = Command::new("claude")
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    Ok(())
}

pub(super) fn mcp_remove(name: &str) -> Result<(), String> {
    let args = build_mcp_remove_args(name);

    let output = Command::new("claude")
        .args(&args)
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(stderr.trim().to_string());
    }

    Ok(())
}

pub(super) fn mcp_check(names: &[String]) -> Result<Vec<McpStatus>, String> {
    let prompt = format!(
        "You have MCP servers registered. For each of the following, call any tool to verify \
         it responds. Reply with only a JSON object matching this shape: \
         {{\"statuses\":[{{\"name\":\"...\",\"ok\":true/false}}]}}. Servers: {}",
        names.join(", ")
    );

    let output = Command::new("claude")
        .args([
            "-p", &prompt,
            "--output-format", "json",
            "--json-schema", CHECK_SCHEMA,
        ])
        .output()
        .map_err(|e| format!("claude: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Claude returns JSON even on failure (is_error in envelope).
    // Try to extract error from JSON before checking exit code.
    parse_check_output(&stdout)
}

fn parse_mcp_names(json: &str) -> HashSet<String> {
    let parsed: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return HashSet::new(),
    };

    parsed
        .get("mcpServers")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

fn build_mcp_add_args(entry: &McpDecl) -> Vec<String> {
    let mut args = vec![
        "mcp".to_string(),
        "add".to_string(),
        "-t".to_string(),
        "http".to_string(),
        "-s".to_string(),
        "user".to_string(),
    ];

    args.push(entry.name.clone());
    args.push(entry.url.clone());

    let mut headers: Vec<(&String, &String)> = entry.headers.iter().collect();
    headers.sort_by_key(|(k, _)| k.as_str());

    for (key, value) in headers {
        args.push("-H".to_string());
        args.push(format!("{key}: {value}"));
    }
    args
}

fn build_mcp_remove_args(name: &str) -> Vec<String> {
    vec![
        "mcp".to_string(),
        "remove".to_string(),
        "-s".to_string(),
        "user".to_string(),
        name.to_string(),
    ]
}

const CHECK_SCHEMA: &str = r#"{"type":"object","properties":{"statuses":{"type":"array","items":{"type":"object","properties":{"name":{"type":"string"},"ok":{"type":"boolean"}},"required":["name","ok"],"additionalProperties":false}}},"required":["statuses"],"additionalProperties":false}"#;

/// Parse Claude's `{"type":"result","result":"..."}` envelope.
fn parse_check_output(output: &str) -> Result<Vec<McpStatus>, String> {
    let parsed: serde_json::Value = serde_json::from_str(output)
        .map_err(|_| "failed to parse claude output".to_string())?;

    // Error results — extract the message
    if parsed.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false) {
        let msg = parsed.get("result")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(format!("claude: {msg}"));
    }

    // result is a JSON string or object containing {"statuses":[...]}
    let result_str = match parsed.get("result") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => return Ok(Vec::new()),
    };

    // Try parsing as {"statuses":[...]} object
    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(&result_str)
        && let Some(statuses) = obj.get("statuses")
    {
        return Ok(super::parse_status_array(&statuses.to_string()));
    }

    // Fallback: bare array
    Ok(super::parse_status_array(&result_str))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

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
    fn session_args_default() {
        let args = build_session_args(&req());
        assert_eq!(args, vec!["--system-prompt".to_string(), "SP".to_string()]);
    }

    #[test]
    fn session_args_resume_replaces_system_prompt() {
        let mut r = req();
        r.resume = true;
        let args = build_session_args(&r);
        assert_eq!(args, vec!["--continue".to_string()]);
    }

    #[test]
    fn session_args_extra_args_come_last() {
        let mut r = req();
        r.extra_args = vec!["--model".to_string(), "opus".to_string()];
        let args = build_session_args(&r);
        let last_two = &args[args.len() - 2..];
        assert_eq!(last_two, ["--model", "opus"]);
    }

    #[test]
    fn one_shot_args_inline_passes_prompt_after_dash_p() {
        let args = build_one_shot_args(&one_shot(PromptInput::Inline("hello".into())));
        assert_eq!(args, vec!["-p".to_string(), "hello".to_string()]);
    }

    #[test]
    fn one_shot_args_stdin_omits_value_after_dash_p() {
        let args = build_one_shot_args(&one_shot(PromptInput::Stdin));
        assert_eq!(args, vec!["-p".to_string()]);
    }

    #[test]
    fn one_shot_args_extra_args_come_last() {
        let mut r = one_shot(PromptInput::Inline("hi".into()));
        r.extra_args = vec!["--model".to_string(), "opus".to_string()];
        let args = build_one_shot_args(&r);
        assert_eq!(args, vec!["-p", "hi", "--model", "opus"]);
    }

    #[test]
    fn parse_mcp_names_extracts_keys() {
        let json = r#"{
            "mcpServers": {
                "linear-server": {"type": "http", "url": "https://mcp.linear.app/mcp"},
                "github": {"type": "http", "url": "https://api.githubcopilot.com/mcp/"}
            }
        }"#;
        let names = parse_mcp_names(json);
        assert_eq!(names.len(), 2);
        assert!(names.contains("linear-server"), "should contain linear-server");
        assert!(names.contains("github"), "should contain github");
    }

    #[test]
    fn parse_mcp_names_missing_field() {
        let names = parse_mcp_names(r#"{"something": "else"}"#);
        assert!(names.is_empty());
    }

    #[test]
    fn parse_mcp_names_empty_servers() {
        let names = parse_mcp_names(r#"{"mcpServers": {}}"#);
        assert!(names.is_empty());
    }

    #[test]
    fn parse_mcp_names_invalid_json() {
        let names = parse_mcp_names("not json");
        assert!(names.is_empty());
    }

    #[test]
    fn build_args_basic() {
        let entry = McpDecl {
            name: "linear".to_string(),
            url: "https://mcp.linear.app/mcp".to_string(),
            headers: std::collections::HashMap::new(),
            instructions: String::new(),
        };

        let args = build_mcp_add_args(&entry);
        assert_eq!(
            args,
            vec!["mcp", "add", "-t", "http", "-s", "user", "linear", "https://mcp.linear.app/mcp"]
        );
    }

    #[test]
    fn build_args_with_header() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer tok".to_string());

        let entry = McpDecl {
            name: "sentry".to_string(),
            url: "https://mcp.sentry.dev/sse".to_string(),
            headers,
            instructions: String::new(),
        };

        let args = build_mcp_add_args(&entry);
        assert_eq!(
            args,
            vec![
                "mcp", "add", "-t", "http", "-s", "user",
                "sentry", "https://mcp.sentry.dev/sse",
                "-H", "Authorization: Bearer tok",
            ]
        );
    }

    // -- parse_check_output --

    #[test]
    fn parse_check_valid() {
        let output = r#"{"type":"result","result":"[{\"name\":\"linear\",\"ok\":true},{\"name\":\"github\",\"ok\":false}]"}"#;
        let result = parse_check_output(output).expect("should parse");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "linear");
        assert!(result[0].ok);
        assert_eq!(result[1].name, "github");
        assert!(!result[1].ok);
    }

    #[test]
    fn parse_check_statuses_object_in_string() {
        let output = r#"{"type":"result","result":"{\"statuses\":[{\"name\":\"linear\",\"ok\":true}]}"}"#;
        let result = parse_check_output(output).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "linear");
        assert!(result[0].ok);
    }

    #[test]
    fn parse_check_statuses_object_direct() {
        let output = r#"{"type":"result","result":{"statuses":[{"name":"linear","ok":true}]}}"#;
        let result = parse_check_output(output).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "linear");
    }

    #[test]
    fn parse_check_bare_array_fallback() {
        // Backwards compat: bare array still works
        let output = r#"{"type":"result","result":"[{\"name\":\"linear\",\"ok\":true}]"}"#;
        let result = parse_check_output(output).expect("should parse");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "linear");
    }

    #[test]
    fn parse_check_malformed_returns_err() {
        assert!(parse_check_output("not json").is_err());
    }

    #[test]
    fn parse_check_empty_object_returns_empty() {
        let result = parse_check_output("{}").expect("valid JSON, no error");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_check_bad_result_string_returns_empty() {
        let result = parse_check_output(r#"{"type":"result","result":"not json"}"#)
            .expect("valid envelope, no is_error");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_check_error_result_returns_err() {
        let output = r#"{"type":"result","subtype":"failure","is_error":true,"result":"Exec failed"}"#;
        let err = parse_check_output(output).expect_err("should be error");
        assert!(err.contains("Exec failed"), "error should contain the message");
    }

    // -- build_mcp_remove_args --

    #[test]
    fn remove_args_basic() {
        let args = build_mcp_remove_args("linear");
        assert_eq!(args, vec!["mcp", "remove", "-s", "user", "linear"]);
    }

    #[test]
    fn build_args_headers_sorted() {
        let mut headers = std::collections::HashMap::new();
        headers.insert("X-Custom".to_string(), "val".to_string());
        headers.insert("Authorization".to_string(), "Bearer tok".to_string());

        let entry = McpDecl {
            name: "test".to_string(),
            url: "https://example.com/mcp".to_string(),
            headers,
            instructions: String::new(),
        };

        let args = build_mcp_add_args(&entry);
        // Positional args must come before -H flags (variadic flag consumes rest)
        let name_pos = args.iter().position(|a| a == "test").unwrap();
        let url_pos = args.iter().position(|a| a == "https://example.com/mcp").unwrap();
        let first_h = args.iter().position(|a| a == "-H").unwrap();

        assert!(name_pos < first_h, "name must precede -H flags");
        assert!(url_pos < first_h, "url must precede -H flags");
        assert_eq!(url_pos, name_pos + 1, "url must follow name");

        let h_positions: Vec<usize> = args
            .iter()
            .enumerate()
            .filter(|(_, a)| *a == "-H")
            .map(|(i, _)| i)
            .collect();

        assert_eq!(h_positions.len(), 2);
        assert_eq!(args[h_positions[0] + 1], "Authorization: Bearer tok");
        assert_eq!(args[h_positions[1] + 1], "X-Custom: val");
    }
}
