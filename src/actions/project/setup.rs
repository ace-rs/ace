use std::ffi::OsStr;
use std::io::IsTerminal;
use std::path::Path;

use super::clone_school::CloneSchool;
use super::edit_config::{EditConfig, FieldEdit};
use super::{PrepareError, Pull};
use crate::ace::{Ace, IoError};
use crate::config;
use crate::school::{linked::LinkedSchool, toml as school_toml};

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("{0}")]
    Config(#[from] config::ConfigError),
    #[error("{0}")]
    Prepare(#[from] PrepareError),
    #[error("{0}")]
    Prompt(#[from] IoError),
    #[error("already set up, use `ace` to run")]
    AlreadySetUp,
}

/// Acquire and validate the linked school before publishing the project's ace.toml.
pub struct Setup<'a> {
    pub specifier: &'a str,
    pub project_dir: &'a Path,
}

impl Setup<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), SetupError> {
        let ace_paths = config::paths::resolve(self.project_dir)?;
        if ace_paths.project.exists() {
            return Err(SetupError::AlreadySetUp);
        }

        if !super::super::is_git_repo(self.project_dir) {
            ace.warn(
                "This directory is not a Git repository; continuing setup without version control.",
            );
        }

        let school = LinkedSchool::resolve(self.project_dir, self.specifier)?;
        if school.needs_clone() {
            self.acquire_school(ace, &school)?;
        } else if school.clone_path.is_some() {
            let outcome = Pull {
                school: &school,
                force: false,
            }
            .run(ace)?;
            outcome.emit(ace);
        }
        school_toml::load(&school.toml_path())?;

        EditConfig {
            path: &ace_paths.project,
            assignments: vec![FieldEdit::new("school", self.specifier)],
        }
        .run(ace)?;
        Ok(())
    }

    fn acquire_school(&self, ace: &mut Ace, school: &LinkedSchool) -> Result<(), SetupError> {
        let clone = CloneSchool { school };
        let error = match clone.run(ace) {
            Ok(()) => return Ok(()),
            Err(PrepareError::RepositoryAccess(message)) => PrepareError::RepositoryAccess(message),
            Err(error) => return Err(error.into()),
        };

        let terminal_prompt = std::env::var_os("GIT_TERMINAL_PROMPT");
        if !ace.can_ask()
            || !std::io::stdin().is_terminal()
            || !git_allows_terminal_prompt(terminal_prompt.as_deref())
        {
            return Err(error.into());
        }

        ace.warn(&error.to_string());
        ace.info("If this repository requires authentication, Git can ask for your GitHub username and a personal access token as the password; GitHub account passwords are not accepted.");
        ace.hint("Create a token with access to this repository: https://github.com/settings/personal-access-tokens/new");
        ace.info("ACE does not store the token. Your configured Git credential helper may store it; without a helper, later commands may need credentials again.");
        let accept = "Enter GitHub credentials";
        let choice = ace.prompt_select(
            "Authenticate with GitHub?",
            vec![accept.to_string(), "Cancel".to_string()],
        )?;
        if choice != accept {
            return Err(IoError::Cancelled.into());
        }
        clone.run_with_credentials(ace)?;
        Ok(())
    }
}

fn git_allows_terminal_prompt(value: Option<&OsStr>) -> bool {
    let Some(value) = value else {
        return true;
    };
    let Some(value) = value.to_str() else {
        return false;
    };
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" => true,
        "false" | "no" | "off" | "" => false,
        _ => value.parse::<i64>().is_ok_and(|number| number != 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_prompt_respects_git_boolean_values() {
        assert!(git_allows_terminal_prompt(None));
        for value in ["true", "TRUE", "yes", "Yes", "on", "ON", "1", "2", "-1"] {
            assert!(
                git_allows_terminal_prompt(Some(OsStr::new(value))),
                "{value}"
            );
        }
        for value in [
            "false", "FALSE", "no", "No", "off", "OFF", "", "0", "-0", "unknown",
        ] {
            assert!(
                !git_allows_terminal_prompt(Some(OsStr::new(value))),
                "{value}"
            );
        }
    }
}
