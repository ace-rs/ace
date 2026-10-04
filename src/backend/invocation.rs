use std::collections::{BTreeMap, HashMap};
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

use super::{Kind, McpDecl, McpStatus, claude, codex, flaude, opencode, pi, pi_mcp};

pub type McpInventory = BTreeMap<String, PathBuf>;

#[derive(Debug, PartialEq, Eq)]
pub enum McpRemoval {
    Removed,
    StillRegistered { source: PathBuf },
}

#[derive(Debug, thiserror::Error)]
pub enum PreflightError {
    #[error("{0}")]
    Unavailable(String),
    #[error("{0}")]
    Failed(String),
    #[error("{0}")]
    InvalidArguments(String),
}

/// Effective backend instance shared by checks, provisioning, and launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub kind: Kind,
    pub command: Vec<String>,
    pub project_dir: PathBuf,
    pub env: HashMap<String, String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    pub extra_args: Vec<String>,
}

impl Invocation {
    pub fn preflight(&self) -> Result<(), PreflightError> {
        match self.kind {
            Kind::Pi => pi::preflight(self),
            Kind::Flaude => flaude::preflight(self),
            _ => Ok(()),
        }
    }

    pub fn mcp_list(&self) -> Result<McpInventory, String> {
        match self.kind {
            Kind::Claude => claude::mcp_list(self),
            Kind::Codex => codex::mcp_list(self),
            Kind::Flaude => flaude::mcp_list(self),
            Kind::OpenCode => opencode::mcp_list(self),
            Kind::Pi => pi_mcp::mcp_list(self),
        }
    }

    pub fn mcp_add(&self, entry: &McpDecl) -> Result<(), String> {
        match self.kind {
            Kind::Claude => claude::mcp_add(entry, self),
            Kind::Codex => codex::mcp_add(entry, self),
            Kind::Flaude => flaude::mcp_add(entry, self),
            Kind::OpenCode => opencode::mcp_add(entry, self),
            Kind::Pi => pi_mcp::mcp_add(entry, self),
        }
    }

    pub fn mcp_remove(&self, name: &str) -> Result<McpRemoval, String> {
        match self.kind {
            Kind::Claude => claude::mcp_remove(name, self),
            Kind::Codex => codex::mcp_remove(name, self),
            Kind::Flaude => flaude::mcp_remove(name, self),
            Kind::OpenCode => opencode::mcp_remove(name, self),
            Kind::Pi => pi_mcp::mcp_remove(name, self),
        }
    }

    pub fn mcp_check(&self, names: &[String]) -> Result<Vec<McpStatus>, String> {
        if names.is_empty() {
            return Ok(Vec::new());
        }
        match self.kind {
            Kind::Claude => claude::mcp_check(names, self),
            Kind::Codex => codex::mcp_check(names, self),
            Kind::Flaude => flaude::mcp_check(names, self),
            Kind::OpenCode => opencode::mcp_check(names, self),
            Kind::Pi => pi_mcp::mcp_check(names, self),
        }
    }

    pub fn process(&self) -> Command {
        let (program, prefix) = self
            .command
            .split_first()
            .map(|(program, prefix)| (program.as_str(), prefix))
            .unwrap_or((self.kind.name(), &[]));
        let mut process = Command::new(program);
        process.args(prefix);
        process.current_dir(&self.project_dir);
        process.envs(&self.env);
        process
    }

    pub fn env_value(&self, name: &str) -> Option<OsString> {
        self.env
            .get(name)
            .map(OsString::from)
            .or_else(|| std::env::var_os(name))
    }

    pub fn home_dir(&self) -> Result<PathBuf, String> {
        crate::paths::home_dir_from(|name| self.env_value(name))
            .ok_or_else(|| "cannot resolve home directory for the backend invocation".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Backend;

    #[test]
    fn invocation_uses_backend_environment_over_resolved_environment() {
        let home_variable = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let mut backend = Backend::from(Kind::Claude);
        backend
            .env
            .insert(home_variable.into(), "/backend-home".into());
        let resolved = HashMap::from([
            (home_variable.into(), "/resolved-home".into()),
            ("ACE_CONTEXT".into(), "shared".into()),
        ]);

        let invocation = backend.invocation(std::path::Path::new("/project"), resolved, vec![]);

        assert_eq!(
            invocation.home_dir().expect("home"),
            PathBuf::from("/backend-home")
        );
        assert_eq!(
            invocation.env_value("ACE_CONTEXT"),
            Some(OsString::from("shared"))
        );
        let process = invocation.process();
        assert_eq!(
            process.get_current_dir(),
            Some(std::path::Path::new("/project"))
        );
    }

    #[cfg(unix)]
    #[test]
    fn legacy_mcp_registration_uses_the_selected_command_and_environment() {
        let temp = tempfile::tempdir().expect("sandbox");
        let project = temp.path().canonicalize().expect("project path");
        let record = project.join("record");
        for kind in [Kind::Claude, Kind::Codex] {
            let mut backend = Backend::from(kind);
            backend.cmd = vec![
                "sh".into(),
                "-c".into(),
                "printf '%s\\n' \"$PWD\" \"$HOME\" \"$@\" > \"$RECORD\"".into(),
                "record-mcp".into(),
            ];
            backend.env = HashMap::from([
                ("HOME".into(), project.to_string_lossy().into_owned()),
                (
                    "CODEX_HOME".into(),
                    project.join("codex").to_string_lossy().into_owned(),
                ),
                ("RECORD".into(), record.to_string_lossy().into_owned()),
            ]);
            let invocation = backend.invocation(&project, HashMap::new(), vec![]);
            let entry = McpDecl {
                name: "example".into(),
                url: "https://example.test/mcp".into(),
                ..McpDecl::default()
            };

            invocation
                .mcp_add(&entry)
                .expect("register through wrapper");

            let recorded = std::fs::read_to_string(&record).expect("wrapper record");
            let lines = recorded.lines().collect::<Vec<_>>();
            assert_eq!(lines[0], project.to_string_lossy());
            assert_eq!(lines[1], project.to_string_lossy());
            assert_eq!(&lines[2..4], &["mcp", "add"]);
            assert!(lines.contains(&"example"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn codex_inventory_uses_the_invocation_config_directory() {
        let temp = tempfile::tempdir().expect("sandbox");
        let project = temp.path().canonicalize().expect("project path");
        let selected = project.join("selected-codex");
        std::fs::create_dir(&selected).expect("Codex directory");
        let config = selected.join("config.toml");
        std::fs::write(
            &config,
            "[mcp_servers.example]\nurl = 'https://example.test/mcp'\n",
        )
        .expect("Codex configuration");
        let mut backend = Backend::from(Kind::Codex);
        backend.cmd = vec!["sh".into(), "-c".into(), "exit 1".into()];
        backend.env = HashMap::from([
            ("HOME".into(), project.to_string_lossy().into_owned()),
            ("CODEX_HOME".into(), selected.to_string_lossy().into_owned()),
        ]);
        let invocation = backend.invocation(&project, HashMap::new(), vec![]);

        let inventory = invocation.mcp_list().expect("fallback inventory");

        assert_eq!(inventory.get("example"), Some(&config));
        std::fs::write(&config, "invalid = =").expect("malformed config");
        let error = invocation
            .mcp_list()
            .expect_err("malformed config must fail");
        assert!(!error.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn codex_inventory_rejects_malformed_cli_output() {
        let temp = tempfile::tempdir().expect("sandbox");
        let mut backend = Backend::from(Kind::Codex);
        backend.cmd = vec!["sh".into(), "-c".into(), "printf 'not JSON'".into()];
        backend.env.insert(
            "CODEX_HOME".into(),
            temp.path().to_string_lossy().into_owned(),
        );
        let invocation = backend.invocation(temp.path(), HashMap::new(), vec![]);

        assert!(
            invocation.mcp_list().is_err(),
            "invalid inventory is not an empty registry"
        );
    }
}
