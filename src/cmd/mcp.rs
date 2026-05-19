use std::collections::HashSet;

use clap::Subcommand;

use crate::ace::Ace;
use crate::backend::McpStatus;
use crate::config::school_toml::McpDecl;
use crate::actions::project::{edit_mcp_config, register_missing_mcp, RegisterMcp, RemoveMcp, register_mcp};

use super::CmdError;

#[derive(Subcommand)]
pub enum Command {
    /// Health-check registered MCP servers (read-only)
    Check,
    /// Remove registered MCP servers, then re-add with `ace mcp`
    Reset {
        /// Specific server name to remove (omit for all school-defined)
        name: Option<String>,
    },
    /// Remove registered MCP servers (alias for reset)
    #[command(hide = true)]
    Clear {
        /// Specific server name to remove (omit for all school-defined)
        name: Option<String>,
    },
    /// Register a single MCP server by name (clears it from `exclude_mcp` if present)
    Register {
        /// Server name (must be defined in the active school)
        name: String,
    },
}

pub fn run(ace: &mut Ace, command: Option<Command>) {
    let result = match command {
        None => run_default(ace),
        Some(Command::Check) => run_check(ace),
        Some(Command::Reset { name } | Command::Clear { name }) => run_reset(ace, name),
        Some(Command::Register { name }) => run_register(ace, name),
    };
    super::exit_on_err(ace, result);
}

/// `ace mcp` — add missing, check health, prompt to re-register broken.
fn run_default(ace: &mut Ace) -> Result<(), CmdError> {
    ace.require_resolved()?;

    let (backend, entries, project_dir) = load_school_mcp(ace)?;
    if entries.is_empty() {
        ace.hint("no MCP servers defined in school");
        return Ok(());
    }

    // -- add missing (prompt per missing entry; "no" appends to exclude_mcp) --

    let local_path = ace.require_paths()?.local.clone();
    register_missing_mcp(ace, &backend, &entries, &project_dir, &local_path)?;

    // -- health check registered servers --

    let registered = backend.mcp_list(&project_dir);
    let check_names: Vec<String> = entries.iter()
        .map(|e| e.name.clone())
        .filter(|n| registered.contains(n))
        .collect();

    if check_names.is_empty() {
        return Ok(());
    }

    ace.progress("Checking MCP server health...");
    let statuses = match backend.mcp_check(&check_names, &project_dir) {
        Ok(s) => s,
        Err(e) => {
            ace.warn(&format!("health check failed: {e}"));
            return Ok(());
        }
    };

    if statuses.is_empty() {
        ace.warn("health check returned no results");
        return Ok(());
    }

    report_statuses(ace, &statuses);

    // -- prompt to re-register broken --

    let broken: Vec<&McpStatus> = statuses.iter().filter(|s| !s.ok).collect();

    for status in &broken {
        let Some(entry) = entries.iter().find(|e| e.name == status.name) else {
            continue;
        };

        let prompt = format!("Re-register '{}'?", status.name);
        if !ace.prompt_confirm(&prompt, true)? {
            continue;
        }

        // Remove and re-add
        if let Err(e) = backend.mcp_remove(&status.name, &project_dir) {
            ace.warn(&format!("remove '{}' failed: {e}", status.name));
            continue;
        }

        let resolved = register_mcp::resolve_headers(entry, ace)?;
        let target = resolved.as_ref().unwrap_or(entry);

        match backend.mcp_add(target, &project_dir) {
            Ok(()) => ace.done(&format!("Re-registered '{}'", status.name)),
            Err(e) => ace.warn(&format!("re-register '{}' failed: {e}", status.name)),
        }
    }

    if broken.is_empty() {
        ace.done("all MCP servers healthy");
    }

    Ok(())
}

/// `ace mcp check` — health check only, no mutations.
fn run_check(ace: &mut Ace) -> Result<(), CmdError> {
    ace.require_resolved()?;

    let (backend, entries, project_dir) = load_school_mcp(ace)?;
    if entries.is_empty() {
        ace.hint("no MCP servers defined in school");
        return Ok(());
    }

    let registered = backend.mcp_list(&project_dir);
    let school_names: HashSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();

    // -- report missing --

    for entry in &entries {
        if !registered.contains(&entry.name) {
            ace.warn(&format!("{} (not registered)", entry.name));
        }
    }

    // -- health check registered --

    let check_names: Vec<String> = entries.iter()
        .map(|e| e.name.clone())
        .filter(|n| registered.contains(n))
        .collect();

    if !check_names.is_empty() {
        ace.progress("Checking MCP server health...");
        match backend.mcp_check(&check_names, &project_dir) {
            Err(e) => ace.warn(&format!("health check failed: {e}")),
            Ok(statuses) if statuses.is_empty() => {
                for name in &check_names {
                    ace.done(&format!("{name} (registered)"));
                }
            }
            Ok(statuses) => report_statuses(ace, &statuses),
        }
    }

    // -- report non-school servers --

    for name in &registered {
        if !school_names.contains(name.as_str()) {
            ace.hint(&format!("{name} (not in school, ignored)"));
        }
    }

    Ok(())
}

/// `ace mcp reset [name]` / `ace mcp clear [name]` — remove servers.
fn run_reset(ace: &mut Ace, name: Option<String>) -> Result<(), CmdError> {
    ace.require_resolved()?;

    let (backend, entries, project_dir) = load_school_mcp(ace)?;
    let registered = backend.mcp_list(&project_dir);

    let names: Vec<String> = match name {
        Some(n) => {
            if !registered.contains(&n) {
                ace.warn(&format!("'{n}' is not registered, nothing to remove"));
                return Ok(());
            }
            vec![n]
        }
        None => {
            let school_registered: Vec<String> = entries.iter()
                .map(|e| e.name.clone())
                .filter(|n| registered.contains(n))
                .collect();

            if school_registered.is_empty() {
                ace.hint("no school-defined MCP servers are registered");
                return Ok(());
            }
            school_registered
        }
    };

    RemoveMcp{ backend: &backend, names: &names, project_dir: &project_dir }.run(ace)
        .map_err(CmdError::Other)?;

    Ok(())
}

/// `ace mcp register <name>` — un-skip and register a single school-defined MCP.
fn run_register(ace: &mut Ace, name: String) -> Result<(), CmdError> {
    ace.require_resolved()?;

    let backend = ace.backend()?.clone();
    let project_dir = ace.project_dir().to_path_buf();

    // Look up the school entry by name (do not apply the exclude filter — we
    // want this to work even when the entry is currently excluded).
    let entry = ace.school()?
        .and_then(|s| s.mcp.iter().find(|e| e.name == name).cloned())
        .ok_or_else(|| CmdError::Other(format!("MCP '{name}' not defined in school")))?;

    let local_path = ace.require_paths()?.local.clone();
    edit_mcp_config::include(&local_path, &name)?;

    let entries = vec![entry];
    RegisterMcp{ backend: &backend, entries: &entries, project_dir: &project_dir }.run(ace)?;
    Ok(())
}

fn report_statuses(ace: &mut Ace, statuses: &[McpStatus]) {
    for status in statuses {
        if status.ok {
            ace.done(&status.name);
        } else {
            ace.error(&format!("{} (unhealthy)", status.name));
        }
    }
}

/// Load school MCP entries and backend from current state. Entries listed in
/// `exclude_mcp` (union across user/project/local scopes) are filtered out
/// before returning.
pub(super) fn load_school_mcp(ace: &Ace) -> Result<(crate::backend::Backend, Vec<McpDecl>, std::path::PathBuf), CmdError> {
    let backend = ace.backend()?.clone();
    let raw = ace.school()?
        .map(|s| s.mcp.clone())
        .unwrap_or_default();
    let excluded = ace.excluded_mcp();
    let entries = filter_excluded(raw, &excluded);
    let project_dir = ace.project_dir().to_path_buf();
    Ok((backend, entries, project_dir))
}

/// Drop entries whose name appears in `excluded`. Order-preserving.
fn filter_excluded(entries: Vec<McpDecl>, excluded: &HashSet<String>) -> Vec<McpDecl> {
    entries.into_iter().filter(|e| !excluded.contains(&e.name)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn decl(name: &str) -> McpDecl {
        McpDecl {
            name: name.to_string(),
            url: format!("https://{name}.example.com/mcp"),
            headers: HashMap::new(),
            instructions: String::new(),
        }
    }

    #[test]
    fn filter_excluded_drops_named() {
        let entries = vec![decl("linear"), decl("github"), decl("sentry")];
        let excluded: HashSet<String> = ["github"].iter().map(|s| s.to_string()).collect();
        let out = filter_excluded(entries, &excluded);
        let names: Vec<&str> = out.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["linear", "sentry"]);
    }

    #[test]
    fn filter_excluded_empty_excludes_returns_all() {
        let entries = vec![decl("linear"), decl("github")];
        let excluded: HashSet<String> = HashSet::new();
        let out = filter_excluded(entries, &excluded);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn filter_excluded_all_excluded_returns_empty() {
        let entries = vec![decl("linear"), decl("github")];
        let excluded: HashSet<String> =
            ["linear", "github"].iter().map(|s| s.to_string()).collect();
        let out = filter_excluded(entries, &excluded);
        assert!(out.is_empty());
    }

    #[test]
    fn filter_excluded_preserves_order() {
        let entries = vec![decl("a"), decl("b"), decl("c"), decl("d")];
        let excluded: HashSet<String> = ["b"].iter().map(|s| s.to_string()).collect();
        let out = filter_excluded(entries, &excluded);
        let names: Vec<&str> = out.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["a", "c", "d"]);
    }
}
