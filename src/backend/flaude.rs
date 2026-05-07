use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Output;

use super::{McpDecl, McpStatus, OneShotRequest, PromptInput, SessionRequest};

pub(super) fn is_ready() -> bool { true }

pub(super) fn exec_session(launch: &[String], req: SessionRequest) -> Result<(), std::io::Error> {
    let Some(path) = exec_record_path() else {
        return Ok(());
    };

    use std::io::Write;

    let record = serde_json::json!({
        "action": "exec_session",
        "trust": req.trust,
        "resume": req.resume,
        "session_prompt": req.session_prompt,
        "project_dir": req.project_dir.to_string_lossy(),
        "extra_args": req.extra_args,
        "cmd": launch,
    });

    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;

    writeln!(file, "{record}")?;
    Ok(())
}

pub(super) fn exec_one_shot(launch: &[String], req: OneShotRequest) -> Result<Output, std::io::Error> {
    use std::io::Write;

    let prompt_repr = match &req.prompt {
        PromptInput::Inline(text) => serde_json::json!({"kind": "inline", "text": text}),
        PromptInput::Stdin => serde_json::json!({"kind": "stdin"}),
    };

    let record = serde_json::json!({
        "action": "exec_one_shot",
        "prompt": prompt_repr,
        "project_dir": req.project_dir.to_string_lossy(),
        "extra_args": req.extra_args,
        "cmd": launch,
    });

    if let Some(path) = exec_record_path() {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        writeln!(file, "{record}")?;
    }

    // Synthesize a successful Output so callers can exercise the capture path.
    // ExitStatus has no cross-platform constructor; spawn a no-op child to mint one.
    let probe = synthesize_success_status()?;
    Ok(Output {
        status: probe,
        stdout: serde_json::to_vec(&record).unwrap_or_default(),
        stderr: Vec::new(),
    })
}

#[cfg(unix)]
fn synthesize_success_status() -> Result<std::process::ExitStatus, std::io::Error> {
    use std::os::unix::process::ExitStatusExt;
    Ok(std::process::ExitStatus::from_raw(0))
}

#[cfg(windows)]
fn synthesize_success_status() -> Result<std::process::ExitStatus, std::io::Error> {
    use std::os::windows::process::ExitStatusExt;
    Ok(std::process::ExitStatus::from_raw(0))
}

pub(super) fn mcp_list() -> HashSet<String> {
    let Some(path) = mcp_list_path() else {
        return HashSet::new();
    };

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return HashSet::new(),
    };

    content
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub(super) fn mcp_add(entry: &McpDecl) -> Result<(), String> {
    use std::io::Write;

    let record_path = mcp_record_path()
        .ok_or_else(|| "HOME not set".to_string())?;

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

pub(super) fn mcp_remove(name: &str) -> Result<(), String> {
    use std::io::Write;

    // -- append removal record --

    let record_path = mcp_record_path()
        .ok_or_else(|| "HOME not set".to_string())?;

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

    let Some(list_path) = mcp_list_path() else {
        return Ok(());
    };

    let existing = std::fs::read_to_string(&list_path).unwrap_or_default();
    let updated: Vec<&str> = existing
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != name)
        .collect();

    std::fs::write(&list_path, updated.join("\n"))
        .map_err(|e| format!("write {}: {e}", list_path.display()))?;

    Ok(())
}

pub(super) fn mcp_check(names: &[String]) -> Result<Vec<McpStatus>, String> {
    Ok(names.iter()
        .map(|n| McpStatus { name: n.clone(), ok: true })
        .collect())
}

/// Flaude exec records file.
fn exec_record_path() -> Option<PathBuf> {
    crate::paths::home_dir().map(|h| h.join(".flaude-exec-records.jsonl"))
}

/// Flaude record file for MCP registrations.
fn mcp_record_path() -> Option<PathBuf> {
    crate::paths::home_dir().map(|h| h.join(".flaude-mcp-records.jsonl"))
}

/// Flaude MCP list file — one server name per line.
fn mcp_list_path() -> Option<PathBuf> {
    crate::paths::home_dir().map(|h| h.join(".flaude-mcp-list"))
}
