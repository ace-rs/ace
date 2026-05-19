use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::ace::{Ace, IoError};
use crate::actions::project::edit_mcp_config;
use crate::backend::{Backend, Kind};
use crate::config::ConfigError;
use crate::config::school_toml::McpDecl;
use crate::templates::Template;

#[derive(Debug, thiserror::Error)]
pub enum RegisterMcpError {
    #[error("mcp register failed: {0}")]
    Register(String),
    #[error("{0}")]
    Io(#[from] IoError),
    #[error("{0}")]
    Config(#[from] ConfigError),
}

/// For each entry: if already registered, pass through. Otherwise prompt
/// `Register MCP '<name>'?` — "yes" batches for registration; "no" appends
/// the name to `exclude_mcp` in `ace.local.toml` so future runs skip it.
pub fn register_missing(
    ace: &mut Ace,
    backend: &Backend,
    entries: &[McpDecl],
    project_dir: &Path,
    local_path: &Path,
) -> Result<(), RegisterMcpError> {
    if entries.is_empty() {
        return Ok(());
    }

    let registered = backend.mcp_list(project_dir);

    let mut to_register: Vec<McpDecl> = Vec::new();
    for entry in entries {
        if registered.contains(&entry.name) {
            to_register.push(entry.clone());
            continue;
        }
        let prompt = format!("Register MCP '{}'?", entry.name);
        if ace.prompt_confirm(&prompt, true)? {
            to_register.push(entry.clone());
        } else {
            edit_mcp_config::exclude(local_path, &entry.name)?;
            ace.hint(&format!("'{}' added to exclude_mcp in ace.local.toml", entry.name));
        }
    }

    if to_register.iter().any(|e| !registered.contains(&e.name)) {
        RegisterMcp { backend, entries: &to_register, project_dir }.run(ace)?;
    }
    Ok(())
}

pub struct RegisterMcp<'a> {
    pub backend: &'a Backend,
    pub entries: &'a [McpDecl],
    pub project_dir: &'a std::path::Path,
}

impl RegisterMcp<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), RegisterMcpError> {
        if self.entries.is_empty() {
            return Ok(());
        }

        // -- check which servers are already registered --

        ace.progress("Checking MCP servers...");
        let registered = self.backend.mcp_list(self.project_dir);

        // -- register missing servers --

        for entry in unregistered(self.entries, &registered) {
            if let Some(hint) = instruction_hint(entry) {
                ace.hint(hint);
            }
            let resolved = resolve_headers(entry, ace)?;
            let target = resolved.as_ref().unwrap_or(entry);

            self.backend.mcp_add(target, self.project_dir)
                .map_err(|e| RegisterMcpError::Register(format!("{}: {e}", entry.name)))?;

            let msg = registration_message(self.backend.kind, &entry.name, entry.headers.is_empty());
            ace.done(&msg);
        }

        Ok(())
    }
}

/// Return entries not yet registered with the backend.
fn unregistered<'a>(entries: &'a [McpDecl], registered: &HashSet<String>) -> Vec<&'a McpDecl> {
    entries
        .iter()
        .filter(|e| !registered.contains(&e.name))
        .collect()
}

fn registration_message(backend: Kind, name: &str, no_headers: bool) -> String {
    if backend == Kind::Codex {
        return format!("Registered MCP server '{name}' — use /mcp inside Codex to finish setup");
    }

    if no_headers {
        format!("Registered MCP server '{name}' — you'll be prompted to authorize on first use")
    } else {
        format!("Registered MCP server '{name}'")
    }
}

/// Parse header values for `{{ placeholder }}` syntax, prompt the user, and return
/// a resolved copy. Returns `None` if no placeholders were found.
pub(crate) fn resolve_headers(entry: &McpDecl, ace: &mut Ace) -> Result<Option<McpDecl>, IoError> {
    // -- collect unique placeholders --

    let all_placeholders = collect_placeholders(&entry.headers);

    if all_placeholders.is_empty() {
        return Ok(None);
    }

    // -- prompt for values --

    let mut values = HashMap::new();
    for name in &all_placeholders {
        let input = ace.prompt_text(&format!("{} ({}):", name, entry.name), None)?;
        values.insert(name.clone(), input);
    }

    // -- substitute into headers --

    let resolved_headers = entry.headers.iter()
        .map(|(k, v)| {
            let tpl = Template::parse(v);
            (k.clone(), tpl.substitute(&values))
        })
        .collect();

    Ok(Some(McpDecl {
        name: entry.name.clone(),
        url: entry.url.clone(),
        headers: resolved_headers,
        instructions: entry.instructions.clone(),
    }))
}

fn instruction_hint(entry: &McpDecl) -> Option<&str> {
    if entry.instructions.is_empty() { None } else { Some(&entry.instructions) }
}

fn collect_placeholders(headers: &HashMap<String, String>) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut placeholders = Vec::new();

    for value in headers.values() {
        for name in Template::parse(value).placeholders() {
            if seen.insert(name.to_string()) {
                placeholders.push(name.to_string());
            }
        }
    }

    placeholders
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(name: &str) -> McpDecl {
        McpDecl {
            name: name.to_string(),
            url: format!("https://{name}.example.com/mcp"),
            headers: HashMap::new(),
            instructions: String::new(),
        }
    }

    // -- unregistered --

    #[test]
    fn unregistered_returns_all_when_none_registered() {
        let entries = vec![decl("linear"), decl("github")];
        let registered = HashSet::new();

        let result = unregistered(&entries, &registered);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn unregistered_returns_empty_when_all_registered() {
        let entries = vec![decl("linear"), decl("github")];
        let registered: HashSet<String> =
            ["linear", "github"].iter().map(|s| s.to_string()).collect();

        let result = unregistered(&entries, &registered);
        assert!(result.is_empty());
    }

    #[test]
    fn unregistered_returns_only_missing() {
        let entries = vec![decl("linear"), decl("github"), decl("sentry")];
        let registered: HashSet<String> = ["linear"].iter().map(|s| s.to_string()).collect();

        let result = unregistered(&entries, &registered);
        let names: Vec<&str> = result.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["github", "sentry"]);
    }

    #[test]
    fn unregistered_empty_entries() {
        let entries: Vec<McpDecl> = vec![];
        let registered: HashSet<String> = ["linear"].iter().map(|s| s.to_string()).collect();

        let result = unregistered(&entries, &registered);
        assert!(result.is_empty());
    }

    // -- registration_message --

    #[test]
    fn message_oauth_mentions_authorize() {
        let msg = registration_message(Kind::Claude, "linear", true);
        assert!(msg.contains("authorize"), "OAuth message should mention authorize prompt");
    }

    #[test]
    fn message_with_headers_omits_authorize() {
        let msg = registration_message(Kind::Claude, "sentry", false);
        assert!(!msg.contains("authorize"), "PAT message should not mention authorize");
        assert!(msg.contains("sentry"));
    }

    #[test]
    fn message_codex_points_to_mcp() {
        let msg = registration_message(Kind::Codex, "linear", true);
        assert!(msg.contains("/mcp"));
        assert!(msg.contains("Codex"));
    }

    // -- instruction_hint --

    #[test]
    fn instruction_hint_returns_some_when_set() {
        let mut entry = decl("github");
        entry.instructions = "do the thing".to_string();
        assert_eq!(instruction_hint(&entry), Some("do the thing"));
    }

    #[test]
    fn instruction_hint_returns_none_when_empty() {
        let entry = decl("linear");
        assert_eq!(instruction_hint(&entry), None);
    }

    #[test]
    fn instruction_hint_works_for_oauth_only_entry() {
        // OAuth-only entries have empty headers; instructions should still surface.
        let mut entry = decl("linear");
        entry.instructions = "authorize in browser".to_string();
        assert!(entry.headers.is_empty());
        assert_eq!(instruction_hint(&entry), Some("authorize in browser"));
    }

    // -- collect_placeholders --

    #[test]
    fn placeholders_none() {
        let headers: HashMap<String, String> =
            [("Authorization".to_string(), "Bearer token123".to_string())]
                .into_iter()
                .collect();

        assert!(collect_placeholders(&headers).is_empty());
    }

    #[test]
    fn placeholders_deduplicates() {
        let headers: HashMap<String, String> = [
            ("X-Key".to_string(), "{{ api_key }}".to_string()),
            ("X-Backup".to_string(), "{{ api_key }}".to_string()),
        ]
        .into_iter()
        .collect();

        let result = collect_placeholders(&headers);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], "api_key");
    }
}
