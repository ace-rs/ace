use crate::ace::{Ace, IoError, StartError};
use crate::actions::migrate::MigrateError;
use crate::actions::project::{PrepareError, RegisterMcpError, SetupError};
use crate::actions::school::{AddImportError, InitError, PullImportsError};
use crate::config::ConfigError;
use crate::git::GitError;

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
    Start(#[from] StartError),
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
            Self::Start(error) => error.hint().map(str::to_string).into_iter().collect(),
            Self::School(error) => error.hint().map(str::to_string).into_iter().collect(),
            Self::Migrate(error) => error.hint().map(str::to_string).into_iter().collect(),
            Self::Prepare(error) => error.hint().map(str::to_string).into_iter().collect(),
            Self::Prompt(error) => error.hint().map(str::to_string).into_iter().collect(),
            Self::Adhoc { hints, .. } => hints.clone(),
            _ => Vec::new(),
        }
    }

    /// Process exit class for this error. Wrapper variants delegate to the
    /// inner error. See `docs/spec/exit-codes.md`.
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::Adhoc { code, .. } => *code,
            Self::Start(error) => start_exit_code(error),
            Self::Prompt(error) => io_exit_code(error),
            Self::Io(_) | Self::Git(_) => ExitCode::Operational,
            Self::Config(error) => config_exit_code(error),
            Self::Backend(error) => backend_exit_code(error),
            Self::School(error) => school_exit_code(error),
            Self::Skill(error) => skill_exit_code(error),
            Self::Setup(error) => setup_exit_code(error),
            Self::Prepare(error) => prepare_exit_code(error),
            Self::McpRegister(error) => mcp_register_exit_code(error),
            Self::Import(error) => add_import_exit_code(error),
            Self::InitSchool(error) => init_exit_code(error),
            Self::PullImports(error) => pull_imports_exit_code(error),
            Self::Migrate(error) => migrate_exit_code(error),
        }
    }
}

fn start_exit_code(error: &StartError) -> ExitCode {
    match error {
        StartError::Io(_) => ExitCode::Operational,
        StartError::Config(error) => config_exit_code(error),
        StartError::Backend(error) => backend_exit_code(error),
        StartError::School(error) => school_exit_code(error),
        StartError::Prepare(error) => prepare_exit_code(error),
        StartError::Prompt(error) => io_exit_code(error),
    }
}

fn io_exit_code(error: &IoError) -> ExitCode {
    match error {
        IoError::Cancelled => ExitCode::Cancelled,
        // Ambient precondition, not something the user mis-typed.
        IoError::NoTerminal { .. } => ExitCode::Unavailable,
        IoError::AskingWaived { .. } | IoError::MachineReadable { .. } => ExitCode::Usage,
        IoError::Io(_) => ExitCode::Operational,
    }
}

fn config_exit_code(error: &ConfigError) -> ExitCode {
    match error {
        // Bad content the user authored in a config file.
        ConfigError::Parse(_)
        | ConfigError::Document(_)
        | ConfigError::InvalidEdit(_)
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

fn migrate_exit_code(error: &MigrateError) -> ExitCode {
    match error {
        // State this binary is too old to read is a missing precondition, not a
        // failed operation — upgrading fixes it.
        MigrateError::FromTheFuture { .. } => ExitCode::Unavailable,
        MigrateError::Config(config) => config_exit_code(config),
        MigrateError::Io(_) => ExitCode::Operational,
    }
}

fn backend_exit_code(error: &crate::backend::BackendError) -> ExitCode {
    use crate::backend::BackendError;

    match error {
        BackendError::TreeLoad(config) => config_exit_code(config),
        BackendError::Unknown(_) => ExitCode::Unavailable,
        BackendError::Unresolvable(_) | BackendError::KindMismatch { .. } => ExitCode::Usage,
    }
}

fn school_exit_code(error: &crate::school::SchoolError) -> ExitCode {
    use crate::school::SchoolError;

    match error {
        SchoolError::TreeLoad(config) => config_exit_code(config),
        SchoolError::NoSpecifier
        | SchoolError::NotInitialized
        | SchoolError::NotCloned
        | SchoolError::NoSchool => ExitCode::Unavailable,
    }
}

fn skill_exit_code(error: &crate::skills::SkillError) -> ExitCode {
    use crate::skills::SkillError;

    match error {
        SkillError::TreeLoad(config) => config_exit_code(config),
        SkillError::School(school) => school_exit_code(school),
        SkillError::Discovery(_) => ExitCode::Operational,
    }
}

fn setup_exit_code(error: &SetupError) -> ExitCode {
    match error {
        SetupError::Config(config) => config_exit_code(config),
        SetupError::NotInGitRepo => ExitCode::Unavailable,
        SetupError::AlreadySetUp => ExitCode::Usage,
    }
}

fn prepare_exit_code(error: &PrepareError) -> ExitCode {
    match error {
        PrepareError::Config(config) => config_exit_code(config),
        PrepareError::Backend(error) => backend_exit_code(error),
        PrepareError::School(error) => school_exit_code(error),
        PrepareError::RegisterMcp(error) => mcp_register_exit_code(error),
        PrepareError::Clone(_) | PrepareError::Write(_) => ExitCode::Operational,
        // The tree is intact; it is waiting on a decision only the user can make.
        PrepareError::BlockedLinks(_) => ExitCode::Unavailable,
    }
}

fn mcp_register_exit_code(error: &RegisterMcpError) -> ExitCode {
    match error {
        RegisterMcpError::Register(_) => ExitCode::Operational,
        RegisterMcpError::Io(prompt) => io_exit_code(prompt),
        RegisterMcpError::Config(config) => config_exit_code(config),
    }
}

fn add_import_exit_code(error: &AddImportError) -> ExitCode {
    match error {
        AddImportError::NoSkills(_) | AddImportError::SkillNotFound(_) => ExitCode::Usage,
        AddImportError::Config(config) => config_exit_code(config),
        AddImportError::Clone(_)
        | AddImportError::Io(_)
        | AddImportError::RejectedImports { .. }
        | AddImportError::BrokenSubmodule(_) => ExitCode::Operational,
    }
}

fn init_exit_code(error: &InitError) -> ExitCode {
    match error {
        InitError::NotInGitRepo => ExitCode::Unavailable,
        InitError::AlreadyExists => ExitCode::Usage,
        InitError::Config(config) => config_exit_code(config),
        InitError::Write(_) => ExitCode::Operational,
        InitError::Pull(error) => pull_imports_exit_code(error),
    }
}

fn pull_imports_exit_code(error: &PullImportsError) -> ExitCode {
    match error {
        PullImportsError::Config(config) => config_exit_code(config),
        PullImportsError::InvalidDecl { .. } => ExitCode::Usage,
        PullImportsError::Io(_)
        | PullImportsError::Git(_)
        | PullImportsError::RejectedImports { .. }
        | PullImportsError::BrokenSubmodules { .. } => ExitCode::Operational,
    }
}

pub(crate) fn exit_on_err(ace: &mut Ace, result: Result<(), CmdError>) {
    if let Err(error) = result {
        let hints = error.hints();
        ace.error(&error.to_string());
        for hint in hints {
            ace.hint(&hint);
        }
        std::process::exit(error.exit_code().code());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git_error() -> GitError {
        GitError::Exec {
            cmd: "status".into(),
            source: std::io::Error::other("boom"),
        }
    }

    #[test]
    fn school_error_hints_delegate_to_leaf() {
        let error = CmdError::School(crate::school::SchoolError::NoSpecifier);
        assert_eq!(
            error.hints(),
            vec!["run `ace setup` to choose a school".to_string()]
        );
    }

    #[test]
    fn school_error_without_leaf_hint_has_none() {
        let inner = crate::school::SchoolError::TreeLoad(ConfigError::NoConfigDir);
        let error = CmdError::School(inner);
        assert!(error.hints().is_empty());
    }

    #[test]
    fn migrate_error_hints_delegate_to_leaf() {
        let error = CmdError::Migrate(MigrateError::FromTheFuture {
            path: std::path::PathBuf::from("/tmp/index.toml"),
            found: "9999-01-01".to_string(),
        });
        assert_eq!(
            error.hints(),
            vec!["upgrade ace to use this install".to_string()]
        );
    }

    #[test]
    fn single_hint_preserves_message() {
        let error = CmdError::failed("boom").with_hint("do the thing");
        assert_eq!(error.to_string(), "boom");
        assert_eq!(error.hints(), vec!["do the thing".to_string()]);
    }

    #[test]
    fn multiple_hints_preserve_order() {
        let error = CmdError::failed("boom").with_hints(vec![
            "first".to_string(),
            "second".to_string(),
            "third".to_string(),
        ]);
        assert_eq!(
            error.hints(),
            vec![
                "first".to_string(),
                "second".to_string(),
                "third".to_string()
            ]
        );
    }

    #[test]
    fn adhoc_error_has_no_hint_by_default() {
        let error = CmdError::failed("plain failure");
        assert!(error.hints().is_empty());
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
        let error = CmdError::unavailable("no school").with_hint("run ace setup");
        assert_eq!(error.exit_code(), ExitCode::Unavailable);
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
        assert_eq!(
            CmdError::Git(git_error()).exit_code(),
            ExitCode::Operational
        );
        assert_eq!(
            CmdError::Prepare(PrepareError::Clone("nope".into())).exit_code(),
            ExitCode::Operational
        );
    }

    #[test]
    fn authored_config_defects_map_to_usage() {
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
        assert_eq!(
            CmdError::Setup(SetupError::Config(ConfigError::NoConfig)).exit_code(),
            ExitCode::Unavailable
        );
    }
}
