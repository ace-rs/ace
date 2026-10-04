mod actions;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Output;

use serde::Deserialize;
use serde::de::DeserializeOwned;

use actions::RecordCall;

use super::{
    Invocation, McpDecl, McpInventory, McpRemoval, McpStatus, OneShotRequest, PreflightError,
    PromptInput, SessionRequest,
};
use crate::config::ace_toml::Trust;
use crate::session::ResumeMode;

pub(super) fn is_ready() -> bool {
    true
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Prerequisite {
    #[default]
    Ready,
    Unavailable(String),
    Failed(String),
    InvalidArguments(String),
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Health {
    #[default]
    Healthy,
    Statuses(BTreeMap<String, bool>),
    Failed(String),
}

pub(super) fn preflight(context: &Invocation) -> Result<(), PreflightError> {
    RecordCall {
        action: "preflight",
        names: &[],
    }
    .run(context)
    .map_err(PreflightError::Failed)?;
    let prerequisite = control(context, "FLAUDE_PREFLIGHT").map_err(PreflightError::Failed)?;

    match prerequisite {
        Prerequisite::Ready => Ok(()),
        Prerequisite::Unavailable(message) => Err(PreflightError::Unavailable(message)),
        Prerequisite::Failed(message) => Err(PreflightError::Failed(message)),
        Prerequisite::InvalidArguments(message) => Err(PreflightError::InvalidArguments(message)),
    }
}

fn control<T: Default + DeserializeOwned>(context: &Invocation, name: &str) -> Result<T, String> {
    let Some(value) = context.env_value(name) else {
        return Ok(T::default());
    };
    let value = value
        .into_string()
        .map_err(|_| format!("{name} is not UTF-8"))?;

    serde_json::from_str(&value).map_err(|error| format!("invalid {name}: {error}"))
}

/// The fixture records the trust level verbatim rather than translating it
/// to flags, so every level round-trips.
pub(super) fn supports_trust(_trust: Trust) -> bool {
    true
}

pub(super) fn exec_session(
    command: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    request: SessionRequest,
) -> Result<(), std::io::Error> {
    let Some(path) = exec_record_path() else {
        return Ok(());
    };

    use std::io::Write;

    let record = serde_json::json!({
        "action": "exec_session",
        "trust": request.trust,
        "resume": matches!(request.resume, ResumeMode::Latest),
        "session_prompt": request.session_prompt,
        "project_dir": request.project_dir.to_string_lossy(),
        "extra_args": request.extra_args,
        "cmd": command,
        "env": request.env,
        "model": model,
        "effort": effort,
    });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    writeln!(file, "{record}")?;
    Ok(())
}

pub(super) fn exec_one_shot(
    command: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    request: OneShotRequest,
) -> Result<Output, std::io::Error> {
    use std::io::Write;

    let prompt_repr = match &request.prompt {
        PromptInput::Inline(text) => serde_json::json!({"kind": "inline", "text": text}),
        PromptInput::Stdin => serde_json::json!({"kind": "stdin"}),
    };

    let record = serde_json::json!({
        "action": "exec_one_shot",
        "prompt": prompt_repr,
        "project_dir": request.project_dir.to_string_lossy(),
        "extra_args": request.extra_args,
        "cmd": command,
        "env": request.env,
        "model": model,
        "effort": effort,
    });

    if let Some(path) = exec_record_path() {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{record}")?;
    }

    // Test hooks for integration tests that need deterministic agent output:
    //   FLAUDE_ONE_SHOT_STDOUT    — replace stdout (default: synthesized JSON record).
    //   FLAUDE_ONE_SHOT_STDERR    — replace stderr (default: empty).
    //   FLAUDE_ONE_SHOT_EXIT_CODE — exit status (default: 0).
    let stdout = match std::env::var("FLAUDE_ONE_SHOT_STDOUT") {
        Ok(v) => v.into_bytes(),
        Err(_) => serde_json::to_vec(&record).unwrap_or_default(),
    };
    let stderr = std::env::var("FLAUDE_ONE_SHOT_STDERR")
        .map(String::into_bytes)
        .unwrap_or_default();
    let exit_code: i32 = std::env::var("FLAUDE_ONE_SHOT_EXIT_CODE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let status = synthesize_status(exit_code)?;

    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

#[cfg(unix)]
fn synthesize_status(code: i32) -> Result<std::process::ExitStatus, std::io::Error> {
    use std::os::unix::process::ExitStatusExt;
    // Unix raw status: code << 8.
    Ok(std::process::ExitStatus::from_raw(code << 8))
}

#[cfg(windows)]
fn synthesize_status(code: i32) -> Result<std::process::ExitStatus, std::io::Error> {
    use std::os::windows::process::ExitStatusExt;
    Ok(std::process::ExitStatus::from_raw(code as u32))
}

pub(super) fn mcp_list(context: &Invocation) -> Result<McpInventory, String> {
    RecordCall {
        action: "mcp_list",
        names: &[],
    }
    .run(context)?;
    let path = context.home_dir()?.join(".flaude-mcp-list");

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(McpInventory::new());
        }
        Err(error) => return Err(format!("read {}: {error}", path.display())),
    };

    Ok(content
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(|name| (name, path.clone()))
        .collect())
}

pub(super) fn mcp_add(entry: &McpDecl, context: &Invocation) -> Result<(), String> {
    RecordCall {
        action: "mcp_add",
        names: std::slice::from_ref(&entry.name),
    }
    .run(context)?;
    use std::io::Write;

    let record_path = context.home_dir()?.join(".flaude-mcp-records.jsonl");

    let mut headers: Vec<(&String, &String)> = entry.headers.iter().collect();
    headers.sort_by_key(|(k, _)| k.as_str());

    let record = serde_json::json!({
        "action": "mcp_add",
        "name": entry.name,
        "url": entry.url,
        "headers": headers.iter()
            .map(|(k, v)| format!("{k}: {v}"))
            .collect::<Vec<_>>(),
    });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&record_path)
        .map_err(|e| format!("open {}: {e}", record_path.display()))?;

    writeln!(file, "{record}").map_err(|e| format!("write {}: {e}", record_path.display()))?;
    Ok(())
}

pub(super) fn mcp_remove(name: &str, context: &Invocation) -> Result<McpRemoval, String> {
    RecordCall {
        action: "mcp_remove",
        names: &[name.to_string()],
    }
    .run(context)?;
    use std::io::Write;

    // -- append removal record --

    let record_path = context.home_dir()?.join(".flaude-mcp-records.jsonl");

    let record = serde_json::json!({
        "action": "mcp_remove",
        "name": name,
    });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&record_path)
        .map_err(|e| format!("open {}: {e}", record_path.display()))?;

    writeln!(file, "{record}").map_err(|e| format!("write {}: {e}", record_path.display()))?;

    // -- update list file --

    let list_path = context.home_dir()?.join(".flaude-mcp-list");
    let existing = match std::fs::read_to_string(&list_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(McpRemoval::Removed);
        }
        Err(error) => return Err(format!("read {}: {error}", list_path.display())),
    };
    let updated: Vec<&str> = existing
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != name)
        .collect();

    std::fs::write(&list_path, updated.join("\n"))
        .map_err(|e| format!("write {}: {e}", list_path.display()))?;

    Ok(McpRemoval::Removed)
}

pub(super) fn mcp_check(names: &[String], context: &Invocation) -> Result<Vec<McpStatus>, String> {
    RecordCall {
        action: "mcp_check",
        names,
    }
    .run(context)?;
    let health = control(context, "FLAUDE_MCP_CHECK")?;

    match health {
        Health::Healthy => Ok(names
            .iter()
            .map(|name| McpStatus {
                name: name.clone(),
                ok: true,
            })
            .collect()),
        Health::Statuses(statuses) => names
            .iter()
            .map(|name| {
                let ok = statuses
                    .get(name)
                    .copied()
                    .ok_or_else(|| format!("Flaude health fixture omitted server '{name}'"))?;
                Ok(McpStatus {
                    name: name.clone(),
                    ok,
                })
            })
            .collect(),
        Health::Failed(message) => Err(message),
    }
}

/// Flaude exec records file.
fn exec_record_path() -> Option<PathBuf> {
    crate::paths::home_dir().map(|h| h.join(".flaude-exec-records.jsonl"))
}
