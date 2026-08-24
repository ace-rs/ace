mod config;
mod diff;
mod explain;
mod fmt;
mod import;
mod link;
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

use crate::ace::{Ace, IoError, WordmarkStyle};
use crate::actions::migrate::MigrateError;
use crate::actions::project::PrepareError;
use crate::actions::project::RegisterMcpError;
use crate::actions::project::SetupError;
use crate::actions::school::InitError;
use crate::actions::school::{AddImportError, PullImportsError};
use crate::config::ace_toml::{AceToml, Trust};
use crate::config::{ConfigError, Scope};
use crate::git::GitError;

pub(crate) const BUILD_IDENTITY: &str =
    concat!(env!("CARGO_PKG_VERSION"), " (", env!("ACE_GIT_HASH"), ")");

#[derive(Parser)]
#[command(
    name = "ace",
    about = "Accelerated Coding Environment",
    version = BUILD_IDENTITY,
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

    /// Answer every checklist with its default instead of asking. Prompts with
    /// no default (free-form and single-choice) fail rather than guess.
    /// Implied by a set `CI` environment variable
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,

    /// Extra arguments passed through to the backend (claude/codex), after --
    #[arg(last = true)]
    backend_args: Vec<String>,
}

#[derive(Subcommand)]
enum Command {
    /// Set up a school (clone + auth + config)
    Setup {
        /// School specifier (`owner/repo`, or the `owner repo` typo).
        /// Omit to link a cached school.
        #[arg(num_args = 0..=2)]
        specifier: Vec<String>,
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
    Link {
        /// Replace symlinks left behind by a previous school
        #[arg(long)]
        force: bool,
    },
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
    /// Print version information
    Version,
}

trait Wordmark {
    fn wordmark(&self) -> WordmarkStyle;
}

impl Wordmark for Command {
    fn wordmark(&self) -> WordmarkStyle {
        match self {
            Self::New { .. } => WordmarkStyle::Big,
            Self::Setup { .. }
            | Self::Fmt
            | Self::Format
            | Self::Import { .. }
            | Self::Pull
            | Self::Link { .. }
            | Self::Auto
            | Self::Yolo => WordmarkStyle::Compact,
            Self::Config {
                command: Some(config::Command::Set { .. }),
            } => WordmarkStyle::Compact,
            Self::Mcp {
                command:
                    Some(
                        mcp::Command::Reset
                        | mcp::Command::Register { .. }
                        | mcp::Command::Unregister { .. },
                    ),
            } => WordmarkStyle::Compact,
            Self::School {
                command: school::Command::Init { .. } | school::Command::Pull,
            } => WordmarkStyle::Compact,
            Self::Skills {
                command:
                    Some(
                        skills::Command::Include { .. }
                        | skills::Command::Exclude { .. }
                        | skills::Command::Reset { .. },
                    ),
                ..
            } => WordmarkStyle::Compact,
            Self::Upgrade { silent: false, .. } => WordmarkStyle::Compact,
            Self::Diff
            | Self::Config { .. }
            | Self::Paths { .. }
            | Self::Mcp { .. }
            | Self::School { .. }
            | Self::Skills { .. }
            | Self::Explain { .. }
            | Self::Upgrade { silent: true, .. }
            | Self::Version => WordmarkStyle::None,
        }
    }
}

impl Cli {
    pub(crate) fn wordmark(&self) -> WordmarkStyle {
        match &self.command {
            None => {
                if self.one_shot_prompt.is_some() {
                    WordmarkStyle::None
                } else {
                    WordmarkStyle::Big
                }
            }
            Some(Command::New { .. }) if self.one_shot_prompt.is_some() => WordmarkStyle::None,
            Some(command) => command.wordmark(),
        }
    }
}

/// Error exit class. Success exits `0` via the normal `main()` return and never
/// flows through `CmdError`, so there is no `Ok` here. See
/// `docs/spec/exit-codes.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExitCode {
    Usage,
    Unavailable,
    Operational,
    Cancelled,
}

impl ExitCode {
    pub(crate) fn code(self) -> i32 {
        match self {
            Self::Usage => 1,
            Self::Unavailable => 2,
            Self::Operational => 3,
            Self::Cancelled => 130,
        }
    }
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
    Migrate(#[from] MigrateError),
    #[error("{0}")]
    Prompt(#[from] IoError),
    /// Ad-hoc error built at a call site. Its exit class is mandatory at
    /// construction (`usage`/`unavailable`/`failed`) — there is no
    /// un-classified catch-all to reach for.
    #[error("{message}")]
    Adhoc {
        message: String,
        hints: Vec<String>,
        code: ExitCode,
    },
}

impl CmdError {
    /// Bad input the user supplied — CLI flags/args or authored config. Exit 1.
    pub fn usage(message: impl Into<String>) -> Self {
        Self::adhoc(message, ExitCode::Usage)
    }

    /// A required resource or precondition is absent. Exit 2.
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self::adhoc(message, ExitCode::Unavailable)
    }

    /// A valid operation was attempted and failed. Exit 3.
    pub fn failed(message: impl Into<String>) -> Self {
        Self::adhoc(message, ExitCode::Operational)
    }

    fn adhoc(message: impl Into<String>, code: ExitCode) -> Self {
        Self::Adhoc {
            message: message.into(),
            hints: Vec::new(),
            code,
        }
    }

    /// Attach a single recovery hint, preserving the error's class.
    pub fn with_hint(self, hint: impl Into<String>) -> Self {
        self.with_hints(vec![hint.into()])
    }

    /// Attach recovery hints, rendered in order, preserving the error's class.
    pub fn with_hints(self, extra: Vec<String>) -> Self {
        match self {
            Self::Adhoc {
                message,
                mut hints,
                code,
            } => {
                hints.extend(extra);
                Self::Adhoc {
                    message,
                    hints,
                    code,
                }
            }
            // Hints only attach to ad-hoc errors; typed variants carry their own.
            other => other,
        }
    }

    /// Recovery hints paired with the error. Empty means no known recovery
    /// action for this variant; callers should not synthesize one.
    pub fn hints(&self) -> Vec<String> {
        match self {
            Self::School(e) => e.hint().map(str::to_string).into_iter().collect(),
            Self::Migrate(e) => e.hint().map(str::to_string).into_iter().collect(),
            Self::Prepare(e) => e.hint().map(str::to_string).into_iter().collect(),
            Self::Prompt(e) => e.hint().map(str::to_string).into_iter().collect(),
            Self::Adhoc { hints, .. } => hints.clone(),
            _ => Vec::new(),
        }
    }

    /// Process exit class for this error. Wrapper variants delegate to the
    /// inner error. See `docs/spec/exit-codes.md`.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Adhoc { code, .. } => *code,
            Self::Prompt(e) => io_exit_code(e),
            Self::Io(_) | Self::Git(_) => ExitCode::Operational,
            Self::Config(e) => config_exit_code(e),
            Self::Backend(e) => backend_exit_code(e),
            Self::School(e) => school_exit_code(e),
            Self::Skill(e) => skill_exit_code(e),
            Self::Setup(e) => setup_exit_code(e),
            Self::Prepare(e) => prepare_exit_code(e),
            Self::McpRegister(e) => mcp_register_exit_code(e),
            Self::Import(e) => add_import_exit_code(e),
            Self::InitSchool(e) => init_exit_code(e),
            Self::PullImports(e) => pull_imports_exit_code(e),
            Self::Migrate(e) => migrate_exit_code(e),
        }
    }
}

// -- Exit-class mapping per leaf error. Wrapper variants delegate inward. --

fn io_exit_code(e: &IoError) -> ExitCode {
    match e {
        IoError::Cancelled => ExitCode::Cancelled,
        // Ambient precondition, not something the user mis-typed.
        IoError::NoTerminal { .. } => ExitCode::Unavailable,
        IoError::AskingWaived { .. } | IoError::MachineReadable { .. } => ExitCode::Usage,
        IoError::Io(_) => ExitCode::Operational,
    }
}

fn config_exit_code(e: &ConfigError) -> ExitCode {
    match e {
        // Bad content the user authored in a config file.
        ConfigError::Parse(_)
        | ConfigError::Encode(_)
        | ConfigError::TraversalInSource(_)
        | ConfigError::TraversalInPath(_) => ExitCode::Usage,
        ConfigError::NoConfig
        | ConfigError::NoConfigDir
        | ConfigError::NoCacheDir
        | ConfigError::NoDataDir => ExitCode::Unavailable,
        ConfigError::Io(_) => ExitCode::Operational,
    }
}

fn migrate_exit_code(e: &MigrateError) -> ExitCode {
    match e {
        // State this binary is too old to read is a missing precondition, not a
        // failed operation — upgrading fixes it.
        MigrateError::FromTheFuture { .. } => ExitCode::Unavailable,
        MigrateError::Config(c) => config_exit_code(c),
        MigrateError::Io(_) => ExitCode::Operational,
    }
}

fn backend_exit_code(e: &crate::backend::BackendError) -> ExitCode {
    use crate::backend::BackendError;
    match e {
        BackendError::TreeLoad(c) => config_exit_code(c),
        BackendError::Unknown(_) => ExitCode::Unavailable,
        BackendError::Unresolvable(_) | BackendError::KindMismatch { .. } => ExitCode::Usage,
    }
}

fn school_exit_code(e: &crate::school::SchoolError) -> ExitCode {
    use crate::school::SchoolError;
    match e {
        SchoolError::TreeLoad(c) => config_exit_code(c),
        SchoolError::NoSpecifier
        | SchoolError::NotInitialized
        | SchoolError::NotCloned
        | SchoolError::NoSchool => ExitCode::Unavailable,
    }
}

fn skill_exit_code(e: &crate::skills::SkillError) -> ExitCode {
    use crate::skills::SkillError;
    match e {
        SkillError::TreeLoad(c) => config_exit_code(c),
        SkillError::School(s) => school_exit_code(s),
        SkillError::Discovery(_) => ExitCode::Operational,
    }
}

fn setup_exit_code(e: &SetupError) -> ExitCode {
    match e {
        SetupError::Config(c) => config_exit_code(c),
        SetupError::NotInGitRepo => ExitCode::Unavailable,
        SetupError::AlreadySetUp => ExitCode::Usage,
    }
}

fn prepare_exit_code(e: &PrepareError) -> ExitCode {
    match e {
        PrepareError::Config(c) => config_exit_code(c),
        PrepareError::Clone(_) | PrepareError::Write(_) => ExitCode::Operational,
        // The tree is intact; it is waiting on a decision only the user can make.
        PrepareError::BlockedLinks(_) => ExitCode::Unavailable,
    }
}

fn mcp_register_exit_code(e: &RegisterMcpError) -> ExitCode {
    match e {
        RegisterMcpError::Register(_) => ExitCode::Operational,
        RegisterMcpError::Io(p) => io_exit_code(p),
        RegisterMcpError::Config(c) => config_exit_code(c),
    }
}

fn add_import_exit_code(e: &AddImportError) -> ExitCode {
    match e {
        AddImportError::NoSkills(_) | AddImportError::SkillNotFound(_) => ExitCode::Usage,
        AddImportError::Config(c) => config_exit_code(c),
        AddImportError::Clone(_)
        | AddImportError::Io(_)
        | AddImportError::RejectedImports { .. }
        | AddImportError::BrokenSubmodule(_) => ExitCode::Operational,
    }
}

fn init_exit_code(e: &InitError) -> ExitCode {
    match e {
        InitError::NotInGitRepo => ExitCode::Unavailable,
        InitError::AlreadyExists => ExitCode::Usage,
        InitError::Config(c) => config_exit_code(c),
        InitError::Write(_) => ExitCode::Operational,
        InitError::Pull(p) => pull_imports_exit_code(p),
    }
}

fn pull_imports_exit_code(e: &PullImportsError) -> ExitCode {
    match e {
        PullImportsError::Config(c) => config_exit_code(c),
        PullImportsError::InvalidDecl { .. } => ExitCode::Usage,
        PullImportsError::Io(_)
        | PullImportsError::Git(_)
        | PullImportsError::RejectedImports { .. }
        | PullImportsError::BrokenSubmodules { .. } => ExitCode::Operational,
    }
}

pub fn run(ace: &mut Ace, cli: Cli) {
    let overrides = match build_overrides(&cli) {
        Ok(o) => o,
        Err(err) => {
            exit_on_err(ace, Err(err));
            return;
        }
    };

    let scopes = scope_override_flags(&cli);
    let scope_override = match scopes.as_slice() {
        [] => None,
        [scope] => Some(*scope),
        _ => {
            exit_on_err(
                ace,
                Err(CmdError::usage(
                    "cannot combine multiple scope flags (--user, --project, --local)",
                )),
            );
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
        Command::Setup { specifier } => {
            // `ace setup prod9 school` arrives as two args; join so the
            // space-separated typo normalizes like the quoted `prod9/school`.
            let specifier = (!specifier.is_empty()).then(|| specifier.join(" "));
            setup::run(ace, specifier.as_deref())
        }
        Command::Import {
            source,
            skill,
            all,
            include_experimental,
            include_system,
        } => import::run(
            ace,
            &source,
            skill.as_deref(),
            all,
            include_experimental,
            include_system,
        ),
        Command::Diff => diff::run(ace),
        Command::Fmt | Command::Format => fmt::run(ace),
        Command::Config { command } => config::run(ace, command),
        Command::Paths { key } => paths::run(ace, key.as_deref()),
        Command::Mcp { command } => mcp::run(ace, command),
        Command::School { command } => school::run(ace, command),
        Command::Skills {
            command,
            all,
            names,
        } => skills::run(ace, command, all, names),
        Command::Explain { name } => explain::run(ace, &name),
        Command::Pull => pull::run(ace),
        Command::Link { force } => link::run(ace, force),
        Command::New { backend_args } => main::run(ace, backend_args, false, cli.one_shot_prompt),
        Command::Auto => yolo::run(ace, crate::config::ace_toml::Trust::Auto),
        Command::Yolo => yolo::run(ace, crate::config::ace_toml::Trust::Yolo),
        Command::Upgrade {
            silent,
            force,
            version,
        } => upgrade::run(ace, silent, force, version),
        Command::Version => {
            println!("ace {BUILD_IDENTITY}");
        }
    }
}

fn scope_override_flags(cli: &Cli) -> Vec<Scope> {
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
    selected
}

fn build_overrides(cli: &Cli) -> Result<AceToml, CmdError> {
    let backends = backend_override_flags(cli);
    let backend = match backends.as_slice() {
        [] => None,
        [b] => Some(b.clone()),
        _ => {
            return Err(CmdError::usage(
                "cannot combine multiple backend override flags",
            ));
        }
    };

    let trusts = trust_override_flags(cli)?;
    let trust = match trusts.as_slice() {
        [] => Trust::default(),
        [t] => *t,
        _ => {
            return Err(CmdError::usage(
                "cannot combine multiple trust override flags (--trust, --auto, --yolo)",
            ));
        }
    };

    Ok(AceToml {
        backend,
        trust,
        session_prompt: cli.session_prompt.clone(),
        env: parse_env_overrides(&cli.env)?,
        ..AceToml::default()
    })
}

fn backend_override_flags(cli: &Cli) -> Vec<String> {
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
    selected
}

fn trust_override_flags(cli: &Cli) -> Result<Vec<Trust>, CmdError> {
    let mut selected: Vec<Trust> = Vec::new();

    if let Some(raw) = &cli.trust {
        selected.push(raw.parse::<Trust>().map_err(CmdError::usage)?);
    }
    if cli.auto {
        selected.push(Trust::Auto);
    }
    if cli.yolo {
        selected.push(Trust::Yolo);
    }

    selected.dedup();
    Ok(selected)
}

fn parse_env_overrides(entries: &[String]) -> Result<HashMap<String, String>, CmdError> {
    let mut out = HashMap::new();
    for entry in entries {
        let (key, value) = entry.split_once('=').ok_or_else(|| {
            CmdError::usage(format!("invalid --env `{entry}` (expected KEY=VAL)"))
        })?;
        if key.is_empty() {
            return Err(CmdError::usage(format!(
                "invalid --env `{entry}` (expected KEY=VAL)"
            )));
        }
        out.insert(key.to_string(), value.to_string());
    }
    Ok(out)
}

pub(crate) fn exit_on_err(ace: &mut Ace, result: Result<(), CmdError>) {
    if let Err(e) = result {
        let hints = e.hints();
        ace.error(&e.to_string());
        for h in hints {
            ace.hint(&h);
        }
        std::process::exit(e.exit_code().code());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_identity_contains_the_package_version_and_commit() {
        assert_eq!(
            BUILD_IDENTITY,
            concat!(env!("CARGO_PKG_VERSION"), " (", env!("ACE_GIT_HASH"), ")")
        );
    }

    fn parses(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).expect("parse command")
    }

    #[test]
    fn session_commands_use_the_big_wordmark() {
        assert_eq!(parses(&["ace"]).wordmark(), WordmarkStyle::Big);
        assert_eq!(parses(&["ace", "new"]).wordmark(), WordmarkStyle::Big);
    }

    #[test]
    fn one_shot_session_uses_no_wordmark() {
        assert_eq!(
            parses(&["ace", "--prompt", "answer"]).wordmark(),
            WordmarkStyle::None
        );
    }

    #[test]
    fn root_mutations_use_the_compact_wordmark() {
        for args in [
            &["ace", "setup"][..],
            &["ace", "fmt"],
            &["ace", "format"],
            &["ace", "import", "owner/repo"],
            &["ace", "pull"],
            &["ace", "link"],
            &["ace", "auto"],
            &["ace", "yolo"],
            &["ace", "upgrade"],
        ] {
            assert_eq!(parses(args).wordmark(), WordmarkStyle::Compact, "{args:?}");
        }
    }

    #[test]
    fn nested_mutations_use_the_compact_wordmark() {
        for args in [
            &["ace", "config", "set", "trust", "auto"][..],
            &["ace", "mcp", "reset"],
            &["ace", "mcp", "register", "docs"],
            &["ace", "mcp", "unregister", "docs"],
            &["ace", "school", "init"],
            &["ace", "school", "pull"],
            &["ace", "skills", "include", "code"],
            &["ace", "skills", "exclude", "code"],
            &["ace", "skills", "reset"],
        ] {
            assert_eq!(parses(args).wordmark(), WordmarkStyle::Compact, "{args:?}");
        }
    }

    #[test]
    fn read_surfaces_use_no_wordmark() {
        for args in [
            &["ace", "diff"][..],
            &["ace", "config"],
            &["ace", "config", "get", "trust"],
            &["ace", "config", "explain"],
            &["ace", "paths"],
            &["ace", "mcp"],
            &["ace", "mcp", "check"],
            &["ace", "school", "skills"],
            &["ace", "school", "validate"],
            &["ace", "skills"],
            &["ace", "explain", "code"],
            &["ace", "upgrade", "--silent"],
            &["ace", "version"],
        ] {
            assert_eq!(parses(args).wordmark(), WordmarkStyle::None, "{args:?}");
        }
    }

    #[test]
    fn root_dash_dash_passthrough() {
        let cli = Cli::try_parse_from(["ace", "--", "-p", "hi"]).expect("parse");
        assert_eq!(cli.backend_args, vec!["-p".to_string(), "hi".to_string()]);
    }

    #[test]
    fn setup_accepts_space_separated_specifier_as_two_args() {
        let cli = Cli::try_parse_from(["ace", "setup", "prod9", "school"]).expect("parse");
        let Some(Command::Setup { specifier }) = cli.command else {
            panic!("expected setup command");
        };
        assert_eq!(specifier, vec!["prod9".to_string(), "school".to_string()]);
    }

    #[test]
    fn setup_rejects_more_than_two_specifier_args() {
        let result = Cli::try_parse_from(["ace", "setup", "a", "b", "c"]);
        assert!(result.is_err(), "3 positional args should be a usage error");
    }

    #[test]
    fn opencode_flag_resolves_to_opencode_backend() {
        let cli = Cli::try_parse_from(["ace", "--opencode"]).expect("parse");
        assert_eq!(backend_override_flags(&cli), vec!["opencode".to_string()]);
    }

    #[test]
    fn cmd_error_hints_school_delegates_to_leaf() {
        let err = CmdError::School(crate::school::SchoolError::NoSpecifier);
        assert_eq!(
            err.hints(),
            vec!["run `ace setup` to choose a school".to_string()]
        );
    }

    #[test]
    fn cmd_error_hints_school_no_hint_when_leaf_returns_none() {
        let inner = crate::school::SchoolError::TreeLoad(ConfigError::NoConfigDir);
        let err = CmdError::School(inner);
        assert!(err.hints().is_empty());
    }

    #[test]
    fn cmd_error_hints_migrate_delegates_to_leaf() {
        let err = CmdError::Migrate(MigrateError::FromTheFuture {
            path: std::path::PathBuf::from("/tmp/index.toml"),
            found: "9999-01-01".to_string(),
        });
        assert_eq!(
            err.hints(),
            vec!["upgrade ace to use this install".to_string()]
        );
    }

    #[test]
    fn cmd_error_with_hint_carries_single_hint() {
        let err = CmdError::failed("boom").with_hint("do the thing");
        assert_eq!(err.to_string(), "boom");
        assert_eq!(err.hints(), vec!["do the thing".to_string()]);
    }

    #[test]
    fn cmd_error_with_hints_preserves_order() {
        let err = CmdError::failed("boom").with_hints(vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ]);
        assert_eq!(
            err.hints(),
            vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string()
            ]
        );
    }

    #[test]
    fn cmd_error_adhoc_has_no_hint_by_default() {
        let err = CmdError::failed("plain failure");
        assert!(err.hints().is_empty());
    }

    // -- exit-code contract (docs/spec/exit-codes.md) --

    fn git_err() -> GitError {
        GitError::Exec {
            cmd: "status".into(),
            source: std::io::Error::other("boom"),
        }
    }

    #[test]
    fn adhoc_constructors_carry_their_class() {
        assert_eq!(CmdError::usage("x").exit_code(), ExitCode::Usage);
        assert_eq!(
            CmdError::unavailable("x").exit_code(),
            ExitCode::Unavailable
        );
        assert_eq!(CmdError::failed("x").exit_code(), ExitCode::Operational);
    }

    #[test]
    fn hints_preserve_the_class() {
        let err = CmdError::unavailable("no school").with_hint("run ace setup");
        assert_eq!(err.exit_code(), ExitCode::Unavailable);
    }

    #[test]
    fn cancellation_maps_to_130() {
        assert_eq!(
            CmdError::Prompt(IoError::Cancelled).exit_code(),
            ExitCode::Cancelled
        );
        assert_eq!(ExitCode::Cancelled.code(), 130);
    }

    #[test]
    fn preconditions_map_to_unavailable() {
        use crate::school::SchoolError;
        assert_eq!(
            CmdError::School(SchoolError::NoSpecifier).exit_code(),
            ExitCode::Unavailable
        );
        assert_eq!(
            CmdError::School(SchoolError::NotInitialized).exit_code(),
            ExitCode::Unavailable
        );
        assert_eq!(
            CmdError::Backend(crate::backend::BackendError::Unknown("x".into())).exit_code(),
            ExitCode::Unavailable
        );
        assert_eq!(
            CmdError::Setup(SetupError::NotInGitRepo).exit_code(),
            ExitCode::Unavailable
        );
        assert_eq!(
            CmdError::Config(ConfigError::NoConfig).exit_code(),
            ExitCode::Unavailable
        );
    }

    #[test]
    fn operations_map_to_operational() {
        assert_eq!(CmdError::Git(git_err()).exit_code(), ExitCode::Operational);
        assert_eq!(
            CmdError::Prepare(PrepareError::Clone("nope".into())).exit_code(),
            ExitCode::Operational
        );
    }

    #[test]
    fn authored_config_defects_map_to_usage() {
        // Decision 3: malformed config the user wrote is their Usage error.
        let parse = toml::from_str::<toml::Table>("x = ").expect_err("bad toml");
        assert_eq!(
            CmdError::Config(ConfigError::Parse(parse)).exit_code(),
            ExitCode::Usage
        );
        assert_eq!(
            CmdError::Config(ConfigError::TraversalInPath("..".into())).exit_code(),
            ExitCode::Usage
        );
        assert_eq!(
            CmdError::Backend(crate::backend::BackendError::Unresolvable("x".into())).exit_code(),
            ExitCode::Usage
        );
    }

    #[test]
    fn wrapper_variants_delegate_to_inner() {
        // Setup wraps Config; the inner code wins, not a fixed outer one.
        assert_eq!(
            CmdError::Setup(SetupError::Config(ConfigError::NoConfig)).exit_code(),
            ExitCode::Unavailable
        );
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
