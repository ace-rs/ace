mod config;
mod diff;
mod error;
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

use crate::ace::{Ace, StartMode, WordmarkStyle};
use crate::backend::{BackendMode, ResumeMode};
use crate::config::Scope;
use crate::config::ace_toml::{AceToml, Trust};

pub(crate) use error::{CmdError, exit_on_err};

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
        let mode = match cli.one_shot_prompt {
            Some(prompt) => StartMode::OneShot { prompt },
            None => StartMode::Session {
                resume: ResumeMode::Latest,
                backend: BackendMode::Normal,
            },
        };
        return main::run(ace, cli.backend_args, mode);
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
        Command::New { backend_args } => {
            let mode = match cli.one_shot_prompt {
                Some(prompt) => StartMode::OneShot { prompt },
                None => StartMode::Session {
                    resume: ResumeMode::Fresh,
                    backend: BackendMode::Normal,
                },
            };
            main::run(ace, backend_args, mode)
        }
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

#[cfg(test)]
mod tests {
    use super::*;

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
