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
    Backend(#[from] crate::backend::BackendError),
    #[error("{0}")]
    Preflight(#[from] crate::backend::PreflightError),
    #[error("{0}")]
    Mcp(#[from] crate::backend::McpError),
    #[error("{0}")]
    Config(#[from] config::ConfigError),
    #[error("{0}")]
    Prepare(#[from] PrepareError),
    #[error("{0}")]
    Prompt(#[from] IoError),
    #[error("already set up, use `ace` to run")]
    AlreadySetUp,
    #[error("no cached schools")]
    NoCachedSchools,
}

/// Which school `ace setup` links.
pub enum SchoolChoice {
    /// A normalized specifier the user named.
    Named(String),
    /// One already downloaded: the only cached school, or the user's pick among several.
    Cached,
}

impl SchoolChoice {
    fn resolve(&self, ace: &mut Ace) -> Result<String, SetupError> {
        let Self::Named(specifier) = self else {
            return Self::pick_cached(ace);
        };
        Ok(specifier.clone())
    }

    fn pick_cached(ace: &mut Ace) -> Result<String, SetupError> {
        let index = config::index_toml::load(&config::index_toml::index_path()?)?;
        let specs = config::index_toml::list_specifiers(&index);

        match specs.as_slice() {
            [] => Err(SetupError::NoCachedSchools),
            [only] => Ok(only.clone()),
            _ => Ok(ace.prompt_select("Select school:", specs)?),
        }
    }
}

/// Acquire and validate the linked school before publishing the project's ace.toml.
/// The already-set-up guard runs before a school is chosen (setup.md § Guards).
pub struct Setup<'a> {
    pub school: SchoolChoice,
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

        let specifier = self.school.resolve(ace)?;
        let school = LinkedSchool::resolve(self.project_dir, &specifier)?;
        let requested_content = self.prepare_school(ace, &school)?;

        let mut pending = match ace.require_tree() {
            Ok(tree) => tree.clone(),
            Err(config::ConfigError::NoConfig) => config::tree::Tree::default(),
            Err(error) => return Err(error.into()),
        };
        pending.project.get_or_insert_default().school = specifier.clone();
        let effective_specifier = pending.specifier().expect("pending project school");
        let effective_school = LinkedSchool::resolve(self.project_dir, &effective_specifier)?;
        let content = if effective_school.root == school.root {
            requested_content
        } else {
            self.prepare_school(ace, &effective_school)?
        };
        let invocation = ace.pending_invocation(&pending, &content, &effective_school.root)?;
        ace.preflight(&invocation)?;
        let excluded = config::selection::Policy::from_tree(&pending)
            .exclude_mcp()
            .values();
        let needs_inventory = content
            .mcp
            .iter()
            .any(|entry| !excluded.contains(&entry.name));
        if needs_inventory {
            invocation.mcp_list()?;
        }

        EditConfig {
            path: &ace_paths.project,
            assignments: vec![FieldEdit::new("school", &specifier)],
        }
        .run(ace)?;
        Ok(())
    }

    fn prepare_school(
        &self,
        ace: &mut Ace,
        school: &LinkedSchool,
    ) -> Result<school_toml::SchoolToml, SetupError> {
        if school.needs_clone() {
            self.acquire_school(ace, school)?;
        } else if school.clone_path.is_some() {
            let outcome = Pull {
                school,
                force: false,
            }
            .run(ace)?;
            outcome.emit(ace);
        }
        Ok(school_toml::load(&school.toml_path())?)
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
