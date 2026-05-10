mod config;
mod diff;
mod explain;
mod link;
mod maverick;
mod fmt;
mod import;
mod learn;
mod main;
mod mcp;
mod paths;
mod pull;
mod school;
mod setup;
mod skills;
mod upgrade;
mod yolo;

use std::collections::HashMap;

use clap::{Parser, Subcommand};

use crate::ace::{Ace, IoError};
use crate::config::ace_toml::{AceToml, Trust};
use crate::config::{ConfigError, Scope};
use crate::actions::school::{AddImportError, PullImportsError};
use crate::actions::project::RegisterMcpError;
use crate::actions::project::PrepareError;
use crate::actions::school::InitError;
use crate::actions::project::{LearnError, SetupError};
use crate::git::GitError;

#[derive(Parser)]
#[command(
    name = "ace",
    about = "Accelerated Coding Environment",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("ACE_GIT_HASH"), ")"),
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Override the configured backend for this command invocation.
    /// Built-ins: claude, codex. Custom names from `[[backends]]`
    /// declarations are also accepted; resolved against the registry.
    #[arg(short = 'b', long, global = true)]
    backend: Option<String>,

    /// Shortcut for `--backend claude`
    #[arg(long, global = true)]
    claude: bool,

    /// Shortcut for `--backend codex`
    #[arg(long, global = true)]
    codex: bool,

    /// Shortcut for `--backend opencode`
    #[arg(long, global = true)]
    opencode: bool,

    /// Shortcut for `--backend flaude` — test-only fixture backend, dev builds only.
    #[cfg(debug_assertions)]
    #[arg(long, global = true, hide = true)]
    flaude: bool,

    /// Trust mode for this invocation (default | auto | yolo).
    /// One-shot override; does not write to disk. Use `ace auto` / `ace yolo`
    /// to persist.
    #[arg(long, global = true, value_name = "MODE")]
    trust: Option<String>,

    /// Shortcut for `--trust auto`. One-shot; does not write to disk.
    /// Use the `auto` subcommand to persist.
    #[arg(long, global = true)]
    auto: bool,

    /// Shortcut for `--trust yolo`. One-shot; does not write to disk.
    /// Use the `yolo` subcommand to persist.
    #[arg(long, global = true)]
    yolo: bool,

    /// Inline session prompt for this invocation. One-shot override.
    #[arg(long, global = true, value_name = "TEXT")]
    session_prompt: Option<String>,

    /// One-shot prompt — run the backend non-interactively, answer this
    /// prompt, and exit. Each backend translates to its native form
    /// (e.g. `claude -p`, `codex exec`).
    #[arg(short = 'p', long = "prompt", global = true, value_name = "TEXT")]
    one_shot_prompt: Option<String>,

    /// Add or override an environment variable for this invocation.
    /// Repeatable: `--env KEY=VAL --env OTHER=VAL`.
    #[arg(long = "env", global = true, value_name = "KEY=VAL")]
    env: Vec<String>,

    /// Write to user-level config (~/.config/ace/ace.toml)
    #[arg(long, global = true)]
    user: bool,

    /// Alias for --user
    #[arg(long = "global", global = true, hide = true)]
    global_alias: bool,

    /// Write to project config (ace.toml)
    #[arg(long, global = true)]
    project: bool,

    /// Write to local config (ace.local.toml)
    #[arg(long, global = true)]
    local: bool,

    /// Machine-readable output (no colors, no spinners, no logo)
    #[arg(long, global = true)]
    pub porcelain: bool,

    /// Extra arguments passed through to the backend (claude/codex), after --
    #[arg(last = true)]
    backend_args: Vec<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Set up a school (clone + auth + config)
    Setup {
        /// School specifier (owner/repo). Omit to link a cached school.
        specifier: Option<String>,
    },
    /// Show uncommitted changes in the school clone
    Diff,
    /// Format ace.toml / school.toml (pretty-print, strip empties)
    Fmt,
    /// Format ace.toml / school.toml (alias for fmt)
    Format,
    /// Print effective configuration, or get/set individual keys
    Config {
        #[command(subcommand)]
        command: Option<config::Command>,
    },
    /// Print resolved filesystem paths ACE uses
    Paths {
        /// Print only this key (e.g. "project", "cache", "school")
        key: Option<String>,
    },
    /// Import a skill from an external repository into the school
    Import {
        /// Skill source (owner/repo or URL)
        source: String,
        /// Specific skill name or glob pattern (e.g. "frontend-*")
        #[arg(long)]
        skill: Option<String>,
        /// Import all skills from the source (equivalent to --skill "*")
        #[arg(long)]
        all: bool,
        /// With --all: also expand into skills/.experimental/
        #[arg(long)]
        include_experimental: bool,
        /// With --all: also expand into skills/.system/
        #[arg(long)]
        include_system: bool,
    },
    /// Manage MCP server registrations
    Mcp {
        #[command(subcommand)]
        command: Option<mcp::Command>,
    },
    /// Manage schools
    School {
        #[command(subcommand)]
        command: school::Command,
    },
    /// List or curate the skills active in this repo
    #[command(visible_alias = "ls")]
    Skills {
        #[command(subcommand)]
        command: Option<skills::Command>,
        /// Show excluded skills too (default: hide)
        #[arg(long)]
        all: bool,
        /// Print bare skill names, one per line
        #[arg(long)]
        names: bool,
    },
    /// Explain how one skill is resolved (provenance + trace)
    Explain {
        /// Skill name to inspect
        name: String,
    },
    /// Fetch latest school changes (force, ignoring cooldown)
    Pull,
    /// Re-link school folders into the project (no pull)
    Link,
    /// Study the project, edit instructions file, narrow `skills` filter
    Learn,
    /// Start a fresh session (skip auto-resume)
    New {
        /// Extra arguments passed through to the backend, after --
        #[arg(last = true)]
        backend_args: Vec<String>,
    },
    /// Enable auto trust mode (AI decides which actions need approval)
    Auto,
    /// Enable yolo trust mode (skip all permission prompts)
    Yolo,
    /// Check for updates and upgrade ACE
    Upgrade {
        /// Suppress all output (used by background spawn)
        #[arg(long)]
        silent: bool,
        /// Reinstall even if at latest, or install a specific version
        #[arg(long)]
        force: bool,
        /// Specific version to install (requires --force)
        version: Option<String>,
    },
    /// 🛩️
    #[command(hide = true)]
    Maverick,
    /// Print version information
    Version,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CmdError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("{0}")]
    Backend(#[from] crate::backend::BackendError),
    #[error("{0}")]
    School(#[from] crate::school::SchoolError),
    #[error("{0}")]
    Skill(#[from] crate::skills::SkillError),
    #[error("{0}")]
    Setup(#[from] SetupError),
    #[error("{0}")]
    Learn(#[from] LearnError),
    #[error("{0}")]
    Prepare(#[from] PrepareError),
    #[error("{0}")]
    McpRegister(#[from] RegisterMcpError),
    #[error("{0}")]
    Import(#[from] AddImportError),
    #[error("{0}")]
    InitSchool(#[from] InitError),
    #[error("{0}")]
    PullImports(#[from] PullImportsError),
    #[error("{0}")]
    Git(#[from] GitError),
    #[error("{0}")]
    Prompt(#[from] IoError),
    #[error("{0}")]
    Other(String),
}

pub fn run(ace: &mut Ace, cli: Cli) {
    let overrides = match build_overrides(&cli) {
        Ok(o) => o,
        Err(err) => {
            exit_on_err(ace, Err(err));
            return;
        }
    };

    let scope_override = match resolve_scope_override(&cli) {
        Ok(scope) => scope,
        Err(err) => {
            exit_on_err(ace, Err(err));
            return;
        }
    };

    ace.set_overrides(overrides);
    ace.set_scope_override(scope_override);

    #[cfg(windows)]
    crate::upgrade::cleanup_old_binary(ace);

    if !cli.porcelain && !matches!(&cli.command, Some(Command::Upgrade { .. })) {
        crate::upgrade::check_for_update(ace);
    }

    let Some(command) = cli.command else {
        return main::run(ace, cli.backend_args, true, cli.one_shot_prompt);
    };

    match command {
        Command::Setup { specifier } => setup::run(ace, specifier.as_deref()),
        Command::Import { source, skill, all, include_experimental, include_system } => {
            import::run(ace, &source, skill.as_deref(), all, include_experimental, include_system)
        }
        Command::Diff => diff::run(ace),
        Command::Fmt | Command::Format => fmt::run(ace),
        Command::Config { command } => config::run(ace, command),
        Command::Paths { key } => paths::run(ace, key.as_deref()),
        Command::Mcp { command } => mcp::run(ace, command),
        Command::School { command } => school::run(ace, command),
        Command::Skills { command, all, names } => skills::run(ace, command, all, names),
        Command::Explain { name } => explain::run(ace, &name),
        Command::Pull => pull::run(ace),
        Command::Link => link::run(ace),
        Command::Learn => learn::run(ace),
        Command::New { backend_args } => main::run(ace, backend_args, false, cli.one_shot_prompt),
        Command::Auto => yolo::run(ace, crate::config::ace_toml::Trust::Auto),
        Command::Yolo => yolo::run(ace, crate::config::ace_toml::Trust::Yolo),
        Command::Upgrade { silent, force, version } => upgrade::run(ace, silent, force, version),
        Command::Maverick => maverick::run(ace),
        Command::Version => {
            println!("ace {}", concat!(env!("CARGO_PKG_VERSION"), " (", env!("ACE_GIT_HASH"), ")"));
        }
    }
}

fn resolve_scope_override(cli: &Cli) -> Result<Option<Scope>, CmdError> {
    let mut selected = Vec::new();

    if cli.user || cli.global_alias {
        selected.push(Scope::User);
    }
    if cli.project {
        selected.push(Scope::Project);
    }
    if cli.local {
        selected.push(Scope::Local);
    }

    selected.dedup();

    match selected.as_slice() {
        [] => Ok(None),
        [scope] => Ok(Some(*scope)),
        _ => Err(CmdError::Other(
            "cannot combine multiple scope flags (--user, --project, --local)".to_string(),
        )),
    }
}

fn build_overrides(cli: &Cli) -> Result<AceToml, CmdError> {
    Ok(AceToml {
        backend: resolve_backend_override(cli)?,
        trust: resolve_trust_override(cli)?.unwrap_or_default(),
        session_prompt: cli.session_prompt.clone(),
        env: parse_env_overrides(&cli.env)?,
        ..AceToml::default()
    })
}

fn resolve_backend_override(cli: &Cli) -> Result<Option<String>, CmdError> {
    let mut selected = Vec::new();

    if let Some(backend) = &cli.backend {
        selected.push(backend.clone());
    }
    if cli.claude {
        selected.push(crate::backend::Kind::Claude.into());
    }
    if cli.codex {
        selected.push(crate::backend::Kind::Codex.into());
    }
    if cli.opencode {
        selected.push(crate::backend::Kind::OpenCode.into());
    }
    #[cfg(debug_assertions)]
    if cli.flaude {
        selected.push(crate::backend::Kind::Flaude.into());
    }

    selected.dedup();

    match selected.as_slice() {
        [] => Ok(None),
        [backend] => Ok(Some(backend.clone())),
        _ => Err(CmdError::Other(
            "cannot combine multiple backend override flags".to_string(),
        )),
    }
}

fn resolve_trust_override(cli: &Cli) -> Result<Option<Trust>, CmdError> {
    let mut selected: Vec<Trust> = Vec::new();

    if let Some(raw) = &cli.trust {
        selected.push(raw.parse::<Trust>().map_err(CmdError::Other)?);
    }
    if cli.auto {
        selected.push(Trust::Auto);
    }
    if cli.yolo {
        selected.push(Trust::Yolo);
    }

    selected.dedup();

    match selected.as_slice() {
        [] => Ok(None),
        [t] => Ok(Some(*t)),
        _ => Err(CmdError::Other(
            "cannot combine multiple trust override flags (--trust, --auto, --yolo)".to_string(),
        )),
    }
}

fn parse_env_overrides(entries: &[String]) -> Result<HashMap<String, String>, CmdError> {
    let mut out = HashMap::new();
    for entry in entries {
        let (key, value) = entry.split_once('=').ok_or_else(|| {
            CmdError::Other(format!("invalid --env `{entry}` (expected KEY=VAL)"))
        })?;
        if key.is_empty() {
            return Err(CmdError::Other(format!(
                "invalid --env `{entry}` (expected KEY=VAL)"
            )));
        }
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

fn exit_on_err(ace: &mut Ace, result: Result<(), CmdError>) {
    if let Err(e) = result {
        ace.error(&e.to_string());
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_dash_dash_passthrough() {
        let cli = Cli::try_parse_from(["ace", "--", "-p", "hi"]).expect("parse");
        assert_eq!(cli.backend_args, vec!["-p".to_string(), "hi".to_string()]);
    }

    #[test]
    fn opencode_flag_resolves_to_opencode_backend() {
        let cli = Cli::try_parse_from(["ace", "--opencode"]).expect("parse");
        let resolved = resolve_backend_override(&cli).expect("resolve");
        assert_eq!(resolved.as_deref(), Some("opencode"));
    }

    #[test]
    fn ace_new_dash_dash_passthrough() {
        let cli = Cli::try_parse_from(["ace", "new", "--", "-p", "hi"]).expect("parse");
        match cli.command {
            Some(Command::New { backend_args }) => {
                assert_eq!(backend_args, vec!["-p".to_string(), "hi".to_string()]);
            }
            _ => panic!("expected Command::New"),
        }
    }
}
