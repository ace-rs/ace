use std::path::Path;

use super::edit_config::{EditConfig, FieldEdit};
use crate::ace::Ace;
use crate::config;

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("{0}")]
    Config(#[from] config::ConfigError),
    #[error("not in git repo, git init?")]
    NotInGitRepo,
    #[error("already set up, use `ace` to run")]
    AlreadySetUp,
}

/// Write ace.toml for the project. Precondition checks only (git repo, not already set up).
pub struct Setup<'a> {
    pub specifier: &'a str,
    pub project_dir: &'a Path,
}

impl Setup<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), SetupError> {
        if !super::super::is_git_repo(self.project_dir) {
            return Err(SetupError::NotInGitRepo);
        }

        let ace_paths = config::paths::resolve(self.project_dir)?;
        if ace_paths.project.exists() {
            return Err(SetupError::AlreadySetUp);
        }

        EditConfig {
            path: &ace_paths.project,
            assignments: vec![FieldEdit::new("school", self.specifier)],
        }
        .run(ace)?;
        Ok(())
    }
}
