use std::collections::HashSet;

use clap::Subcommand;

use crate::ace::Ace;
use crate::actions::project::edit_mcp_config::{EditMcpConfig, Op};
use crate::actions::project::{RegisterMcp, RemoveMcp};
use crate::backend::McpStatus;
use crate::school::toml::McpDecl;

use super::CmdError;

#[derive(Subcommand)]
pub enum Command {
    /// Health-check registered MCP servers (read-only)
    Check,
    /// Remove every registered school-defined MCP server
    Reset,
    /// Register a single MCP server by name (clears it from `exclude_mcp` if present)
    Register {
        /// Server name (must be defined in the linked school)
        name: String,
    },
    /// Remove a single registered MCP server by name
    #[command(visible_alias = "remove")]
    Unregister {
        /// Server name
        name: String,
    },
}

pub fn run(ace: &mut Ace, command: Option<Command>) {
    let result = match command {
        None => run_list(ace),
        Some(Command::Check) => run_check(ace),
        Some(Command::Reset) => run_reset(ace),
        Some(Command::Register { name }) => run_register(ace, name),
        Some(Command::Unregister { name }) => run_unregister(ace, name),
    };
    super::exit_on_err(ace, result);
}

/// `ace mcp` — read-only inventory: what the school declares against what the
/// backend has. Deliberately does not probe health; that costs a subprocess
/// round-trip per server and belongs to `ace mcp check`.
fn run_list(ace: &mut Ace) -> Result<(), CmdError> {
    ace.require_config()?;

    let backend = ace.backend()?.clone();
    let declared = match ace.school() {
        Ok(s) => s.mcp.clone(),
        Err(e) if e.is_absent() => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let excluded = ace.excluded_mcp();
    let registered = backend.mcp_list(ace.project_dir());

    for row in inventory(&declared, &excluded, &registered) {
        ace.data(&format!("{}\t{}", row.name, row.state.label()));
    }

    Ok(())
}

/// Where a server stands relative to the school and the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum McpState {
    Registered,
    Missing,
    Excluded,
    Foreign,
}

impl McpState {
    fn label(self) -> &'static str {
        match self {
            Self::Registered => "registered",
            Self::Missing => "not registered",
            Self::Excluded => "excluded",
            Self::Foreign => "not in linked school",
        }
    }
}

struct McpRow {
    name: String,
    state: McpState,
}

/// School order first (an exclusion outranks registration — it is the user's
/// standing decision), then backend servers the school never declared.
fn inventory(
    declared: &[McpDecl],
    excluded: &HashSet<String>,
    registered: &HashSet<String>,
) -> Vec<McpRow> {
    let school_names: HashSet<&str> = declared.iter().map(|e| e.name.as_str()).collect();

    let mut rows: Vec<McpRow> = declared
        .iter()
        .map(|entry| {
            let state = match (
                excluded.contains(&entry.name),
                registered.contains(&entry.name),
            ) {
                (true, _) => McpState::Excluded,
                (false, true) => McpState::Registered,
                (false, false) => McpState::Missing,
            };
            McpRow {
                name: entry.name.clone(),
                state,
            }
        })
        .collect();

    let mut foreign: Vec<&String> = registered
        .iter()
        .filter(|n| !school_names.contains(n.as_str()))
        .collect();
    foreign.sort();

    rows.extend(foreign.into_iter().map(|name| McpRow {
        name: name.clone(),
        state: McpState::Foreign,
    }));
    rows
}

/// `ace mcp check` — health check only, no mutations.
fn run_check(ace: &mut Ace) -> Result<(), CmdError> {
    ace.require_config()?;

    let (backend, entries, project_dir) = load_school_mcp(ace)?;
    if entries.is_empty() {
        ace.info("no MCP servers defined in school");
        return Ok(());
    }

    let registered = backend.mcp_list(&project_dir);
    let school_names: HashSet<&str> = entries.iter().map(|e| e.name.as_str()).collect();

    // -- report missing --

    for entry in &entries {
        if !registered.contains(&entry.name) {
            ace.warn(&format!("{} (not registered)", entry.name));
            ace.hint(&format!(
                "run `ace mcp register {}` to register it",
                entry.name
            ));
        }
    }

    // -- health check registered --

    let check_names: Vec<String> = entries
        .iter()
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
            ace.info(&format!("{name} (not in linked school, ignored)"));
        }
    }

    Ok(())
}

/// `ace mcp reset` — remove every registered school-defined server.
fn run_reset(ace: &mut Ace) -> Result<(), CmdError> {
    ace.require_config()?;

    let (backend, entries, project_dir) = load_school_mcp(ace)?;
    let registered = backend.mcp_list(&project_dir);

    let names: Vec<String> = entries
        .iter()
        .map(|e| e.name.clone())
        .filter(|n| registered.contains(n))
        .collect();

    if names.is_empty() {
        ace.info("no school-defined MCP servers are registered");
        return Ok(());
    }

    RemoveMcp {
        backend: &backend,
        names: &names,
        project_dir: &project_dir,
    }
    .run(ace)
    .map_err(CmdError::failed)
}

/// `ace mcp unregister <name>` — remove one registered server.
fn run_unregister(ace: &mut Ace, name: String) -> Result<(), CmdError> {
    ace.require_config()?;

    let (backend, _, project_dir) = load_school_mcp(ace)?;

    if !backend.mcp_list(&project_dir).contains(&name) {
        ace.info(&format!("'{name}' is not registered, nothing to remove"));
        return Ok(());
    }

    RemoveMcp {
        backend: &backend,
        names: &[name],
        project_dir: &project_dir,
    }
    .run(ace)
    .map_err(CmdError::failed)
}

/// `ace mcp register <name>` — un-skip and register a single school-defined MCP.
fn run_register(ace: &mut Ace, name: String) -> Result<(), CmdError> {
    ace.require_config()?;

    let backend = ace.backend()?.clone();
    let project_dir = ace.project_dir().to_path_buf();

    // Look up the school entry by name (do not apply the exclude filter — we
    // want this to work even when the entry is currently excluded). An absent
    // school reads the same as a name it doesn't define.
    let school_mcp = match ace.school() {
        Ok(s) => s.mcp.clone(),
        Err(e) if e.is_absent() => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let entry = school_mcp
        .iter()
        .find(|e| e.name == name)
        .cloned()
        .ok_or_else(|| {
            CmdError::usage(format!("MCP '{name}' not defined in school"))
                .with_hint("run `ace mcp`")
        })?;

    let local_path = ace.paths().local.clone();
    EditMcpConfig {
        path: &local_path,
        op: Op::Include(name),
    }
    .run(ace)?;

    let entries = vec![entry];
    RegisterMcp {
        backend: &backend,
        entries: &entries,
        project_dir: &project_dir,
    }
    .run(ace)?;
    Ok(())
}

fn report_statuses(ace: &mut Ace, statuses: &[McpStatus]) {
    for status in statuses {
        if status.ok {
            ace.done(&status.name);
        } else {
            ace.error(&format!("{} (unhealthy)", status.name));
            ace.hint("try /mcp");
        }
    }
}

/// Load school MCP entries and backend from current state. Entries listed in
/// `exclude_mcp` (union across user/project/local scopes) are filtered out
/// before returning.
pub(super) fn load_school_mcp(
    ace: &Ace,
) -> Result<(crate::backend::Backend, Vec<McpDecl>, std::path::PathBuf), CmdError> {
    let backend = ace.backend()?.clone();
    let raw = match ace.school() {
        Ok(s) => s.mcp.clone(),
        Err(e) if e.is_absent() => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let excluded = ace.excluded_mcp();
    let entries = filter_excluded(raw, &excluded);
    let project_dir = ace.project_dir().to_path_buf();
    Ok((backend, entries, project_dir))
}

/// Drop entries whose name appears in `excluded`. Order-preserving.
fn filter_excluded(entries: Vec<McpDecl>, excluded: &HashSet<String>) -> Vec<McpDecl> {
    entries
        .into_iter()
        .filter(|e| !excluded.contains(&e.name))
        .collect()
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

    // -- inventory --

    fn names_of(rows: &[McpRow]) -> Vec<(&str, McpState)> {
        rows.iter().map(|r| (r.name.as_str(), r.state)).collect()
    }

    fn set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn inventory_classifies_each_declared_server() {
        let declared = vec![decl("linear"), decl("github"), decl("sentry")];
        let rows = inventory(&declared, &set(&["sentry"]), &set(&["linear"]));

        assert_eq!(
            names_of(&rows),
            vec![
                ("linear", McpState::Registered),
                ("github", McpState::Missing),
                ("sentry", McpState::Excluded),
            ]
        );
    }

    #[test]
    fn inventory_marks_exclusion_over_registration() {
        let declared = vec![decl("linear")];
        let rows = inventory(&declared, &set(&["linear"]), &set(&["linear"]));
        assert_eq!(names_of(&rows), vec![("linear", McpState::Excluded)]);
    }

    #[test]
    fn inventory_appends_foreign_servers_sorted() {
        let declared = vec![decl("linear")];
        let rows = inventory(
            &declared,
            &HashSet::new(),
            &set(&["zed", "linear", "atlas"]),
        );

        assert_eq!(
            names_of(&rows),
            vec![
                ("linear", McpState::Registered),
                ("atlas", McpState::Foreign),
                ("zed", McpState::Foreign),
            ]
        );
    }

    #[test]
    fn inventory_empty_school_lists_only_foreign() {
        let rows = inventory(&[], &HashSet::new(), &set(&["atlas"]));
        assert_eq!(names_of(&rows), vec![("atlas", McpState::Foreign)]);
    }

    // -- filter_excluded --

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
