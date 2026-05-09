mod claude;
mod codex;
mod flaude;
mod opencode;
pub mod registry;

use crate::config::ConfigError;

/// Errors that can occur while binding a `Resolved` view to a concrete
/// `Backend` — including pre-binding tree/merge failures bubbled through
/// `ConfigError`.
#[derive(Debug, thiserror::Error)]
pub enum BackendError {
    #[error(transparent)]
    TreeLoad(#[from] ConfigError),
    #[error("unknown backend: {0}")]
    Unknown(String),
    #[error("cannot resolve kind for custom backend `{0}`: set `kind = \"...\"` or use a `cmd` whose binary matches a built-in")]
    Unresolvable(String),
    #[error("backend `{name}` declared kind `{declared}` but is already registered as `{actual}`")]
    KindMismatch {
        name: String,
        declared: String,
        actual: String,
    },
}

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Output;

use serde::{Deserialize, Serialize};

use crate::config::ace_toml::Trust;
use crate::config::school_toml::McpDecl;

/// Inputs to an interactive session launch — exec-replace transport.
///
/// `cmd` (the launch argv) is *not* in here — it's a property of the
/// backend instance, not session input. Per-backend `exec_session` takes
/// it as a separate parameter, populated by `Backend::exec_session` from
/// `self.cmd`.
pub struct SessionRequest {
    pub trust: Trust,
    pub session_prompt: String,
    pub project_dir: PathBuf,
    pub env: HashMap<String, String>,
    pub extra_args: Vec<String>,
    pub resume: bool,
}

/// Inputs to a one-shot (non-interactive) launch — spawn-and-capture transport.
///
/// No `trust`, `session_prompt`, or `resume` — the non-interactive entry point
/// doesn't need approval modes or system-prompt injection. See
/// `spec/decisions/009-polymorphic-flags.md`.
pub struct OneShotRequest {
    pub prompt: PromptInput,
    pub project_dir: PathBuf,
    pub env: HashMap<String, String>,
    pub extra_args: Vec<String>,
}

/// Source of the one-shot prompt. `Inline` passes the prompt as argv;
/// `Stdin` lets the spawned child inherit the parent's stdin.
#[derive(Debug, Clone)]
pub enum PromptInput {
    Inline(String),
    Stdin,
}

/// Health check result for a single MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpStatus {
    pub name: String,
    pub ok: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    #[default]
    Claude,
    Codex,
    Flaude,
    OpenCode,
}

/// Dispatch a method call to the matching backend module's free function.
macro_rules! dispatch {
    ($self:expr, $method:ident $(, $arg:expr)*) => {
        match $self {
            Kind::Claude => claude::$method($($arg),*),
            Kind::Codex => codex::$method($($arg),*),
            Kind::Flaude => flaude::$method($($arg),*),
            Kind::OpenCode => opencode::$method($($arg),*),
        }
    };
}

impl Kind {
    /// Built-in backends surfaced to users. `Flaude` is a test-only fixture
    /// backend and is excluded from release builds — its variant remains in
    /// the enum so internal code paths still compile, but it is unreachable
    /// via name lookup or the registry in `cargo build --release`.
    #[cfg(debug_assertions)]
    pub const ALL: &'static [Kind] = &[Kind::Claude, Kind::Codex, Kind::OpenCode, Kind::Flaude];

    #[cfg(not(debug_assertions))]
    pub const ALL: &'static [Kind] = &[Kind::Claude, Kind::Codex, Kind::OpenCode];

    /// Canonical name. Doubles as registry key for built-in entries and as the
    /// default `cmd[0]` (the binary name) when no override is provided.
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Claude => "claude",
            Kind::Codex => "codex",
            Kind::Flaude => "flaude",
            Kind::OpenCode => "opencode",
        }
    }

    /// Lookup a built-in kind by canonical name.
    pub fn from_name(name: &str) -> Option<Kind> {
        Kind::ALL.iter().copied().find(|k| k.name() == name)
    }

    pub fn backend_dir(&self) -> &'static str {
        match self {
            Kind::Claude | Kind::Flaude => ".claude",
            Kind::Codex => ".agents",
            Kind::OpenCode => ".opencode",
        }
    }

    pub fn instructions_file(&self) -> &'static str {
        match self {
            Kind::Claude | Kind::Flaude => "CLAUDE.md",
            Kind::Codex | Kind::OpenCode => "AGENTS.md",
        }
    }

    /// Whether this backend natively supports a linked folder type.
    pub fn is_folder_supported(&self, folder: &str) -> bool {
        matches!(
            (self, folder),
            (_, "skills")
                | (Kind::Claude | Kind::Flaude, _)
                | (Kind::OpenCode, "commands" | "agents")
        )
    }

    pub fn exec_session(&self, cmd: &[String], req: SessionRequest) -> Result<(), std::io::Error> {
        dispatch!(self, exec_session, cmd, req)
    }

    pub fn exec_one_shot(&self, cmd: &[String], req: OneShotRequest) -> Result<Output, std::io::Error> {
        dispatch!(self, exec_one_shot, cmd, req)
    }

    #[allow(dead_code)]
    pub fn is_ready(&self) -> bool {
        dispatch!(self, is_ready)
    }

    pub fn mcp_list(&self, project_dir: &Path) -> HashSet<String> {
        dispatch!(self, mcp_list, project_dir)
    }

    pub fn mcp_remove(&self, name: &str, project_dir: &Path) -> Result<(), String> {
        dispatch!(self, mcp_remove, name, project_dir)
    }

    pub fn mcp_check(&self, names: &[String], project_dir: &Path) -> Result<Vec<McpStatus>, String> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        dispatch!(self, mcp_check, names, project_dir)
    }

    pub fn mcp_add(&self, entry: &McpDecl, project_dir: &Path) -> Result<(), String> {
        dispatch!(self, mcp_add, entry, project_dir)
    }
}

/// Convert a `Kind` to its canonical name. Equivalent to
/// `k.name().to_string()`; provided so callsites can use `.into()` when
/// pushing a kind into a `String` field.
impl From<Kind> for String {
    fn from(k: Kind) -> String {
        k.name().to_string()
    }
}

/// Construct a `Backend` instance defaulted from a `Kind`: `name`/`cmd[0]`
/// = canonical name, empty env.
impl From<Kind> for Backend {
    fn from(kind: Kind) -> Backend {
        Backend {
            name: kind.name().to_string(),
            kind,
            cmd: vec![kind.name().to_string()],
            env: HashMap::new(),
        }
    }
}

impl Default for Backend {
    fn default() -> Backend {
        Kind::default().into()
    }
}

/// A resolved backend instance: identity (`name`), behavior (`kind`), and runtime
/// overrides (`cmd`, `env`). Built-ins are pre-built singletons; custom entries
/// from `[[backends]]` populate the registry alongside built-ins.
#[derive(Debug, Clone)]
pub struct Backend {
    pub name: String,
    pub kind: Kind,
    /// Argv for launching the binary. Built-ins seed `[kind.name()]`; custom
    /// backends from `[[backends]]` override.
    pub cmd: Vec<String>,
    pub env: HashMap<String, String>,
}

impl Backend {
    pub fn backend_dir(&self) -> &'static str {
        self.kind.backend_dir()
    }

    pub fn instructions_file(&self) -> &'static str {
        self.kind.instructions_file()
    }

    pub fn exec_session(&self, mut req: SessionRequest) -> Result<(), std::io::Error> {
        // per-backend env merges over global env (later wins on collision).
        for (k, v) in &self.env {
            req.env.insert(k.clone(), v.clone());
        }
        self.kind.exec_session(&self.cmd, req)
    }

    pub fn exec_one_shot(&self, mut req: OneShotRequest) -> Result<Output, std::io::Error> {
        for (k, v) in &self.env {
            req.env.insert(k.clone(), v.clone());
        }
        self.kind.exec_one_shot(&self.cmd, req)
    }

    pub fn mcp_list(&self, project_dir: &Path) -> HashSet<String> {
        self.kind.mcp_list(project_dir)
    }

    pub fn mcp_remove(&self, name: &str, project_dir: &Path) -> Result<(), String> {
        self.kind.mcp_remove(name, project_dir)
    }

    pub fn mcp_check(&self, names: &[String], project_dir: &Path) -> Result<Vec<McpStatus>, String> {
        self.kind.mcp_check(names, project_dir)
    }

    pub fn mcp_add(&self, entry: &McpDecl, project_dir: &Path) -> Result<(), String> {
        self.kind.mcp_add(entry, project_dir)
    }
}

/// Name → Backend lookup. Built with `Registry::with_builtins()`; layer-merge
/// from `[[backends]]` happens in `registry::build_registry`.
#[derive(Debug, Default, Clone)]
pub struct Registry {
    entries: HashMap<String, Backend>,
}

impl Registry {
    pub fn with_builtins() -> Self {
        let entries = Kind::ALL.iter()
            .map(|k| (k.name().to_string(), Backend::from(*k)))
            .collect();
        Self { entries }
    }

    pub fn lookup(&self, name: &str) -> Option<&Backend> {
        self.entries.get(name)
    }

    pub(crate) fn get_mut(&mut self, name: &str) -> Option<&mut Backend> {
        self.entries.get_mut(name)
    }

    pub(crate) fn insert(&mut self, backend: Backend) {
        self.entries.insert(backend.name.clone(), backend);
    }
}

/// Parse `[{"name":"...","ok":bool}]` JSON into McpStatus vec.
/// Shared helper — each backend extracts the JSON string from its own output format,
/// then calls this to parse the common shape.
pub(super) fn parse_status_array(json: &str) -> Vec<McpStatus> {
    #[derive(serde::Deserialize)]
    struct Entry {
        name: String,
        ok: bool,
    }

    let entries: Vec<Entry> = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };

    entries.into_iter()
        .map(|e| McpStatus { name: e.name, ok: e.ok })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Kind, Registry};

    #[test]
    fn registry_with_builtins_lookup() {
        let registry = Registry::with_builtins();

        let claude = registry.lookup("claude").expect("claude builtin");
        assert_eq!(claude.kind, Kind::Claude);
        assert_eq!(claude.name, "claude");

        let codex = registry.lookup("codex").expect("codex builtin");
        assert_eq!(codex.kind, Kind::Codex);

        let opencode = registry.lookup("opencode").expect("opencode builtin");
        assert_eq!(opencode.kind, Kind::OpenCode);

        let flaude = registry.lookup("flaude").expect("flaude builtin");
        assert_eq!(flaude.kind, Kind::Flaude);

        assert!(registry.lookup("unknown").is_none());
    }
}
