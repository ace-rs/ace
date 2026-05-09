use std::path::Path;

use crate::ace::Ace;
use crate::actions::school::pull_imports::{PullImports, PullImportsError};
use crate::config::school_toml::{self, ImportDecl};
use crate::config::ConfigError;
use crate::templates;

/// Default school imported by every fresh school. Provides `ace-school` and
/// any other base skills. Users may remove the import for a fully
/// standalone school. See `docs/spec/school/standard-imports.md`.
const STANDARD_SCHOOL_SOURCE: &str = "ace-rs/school";

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("not in git repo, git init?")]
    NotInGitRepo,
    #[error("school.toml already exists")]
    AlreadyExists,
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("write failed: {0}")]
    Write(std::io::Error),
    #[error("{0}")]
    Pull(#[from] PullImportsError),
}

pub struct Init<'a> {
    pub name: &'a str,
    pub project_dir: &'a Path,
    pub force: bool,
}

impl Init<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), InitError> {
        if !super::super::is_git_repo(self.project_dir) {
            return Err(InitError::NotInGitRepo);
        }

        let toml_path = self.project_dir.join("school.toml");
        if !self.force && toml_path.exists() {
            return Err(InitError::AlreadyExists);
        }

        if self.force && toml_path.exists() {
            let mut toml = school_toml::load(&toml_path)?;
            toml.name = self.name.to_string();
            ensure_standard_import(&mut toml);
            school_toml::save(&toml_path, &toml)?;
        } else {
            let mut toml = school_toml::SchoolToml {
                name: self.name.to_string(),
                ..Default::default()
            };
            ensure_standard_import(&mut toml);
            school_toml::save(&toml_path, &toml)?;
        }
        ace.done("Created school.toml");

        let vals = std::collections::HashMap::from([
            ("school_name".to_string(), self.name.to_string()),
        ]);

        let instructions = self.project_dir.join("CLAUDE.md");
        if !instructions.exists() {
            let tpl = templates::Template::parse(templates::builtins::SCHOOL_CLAUDE_MD);
            std::fs::write(&instructions, tpl.substitute(&vals))
                .map_err(InitError::Write)?;
            ace.done("Created CLAUDE.md");
        }

        let readme = self.project_dir.join("README.md");
        if !readme.exists() {
            let tpl = templates::Template::parse(templates::builtins::SCHOOL_README);
            std::fs::write(&readme, tpl.substitute(&vals))
                .map_err(InitError::Write)?;
            ace.done("Created README.md");
        }

        let gitignore = self.project_dir.join(".gitignore");
        if !gitignore.exists() {
            std::fs::write(&gitignore, templates::builtins::GITIGNORE)
                .map_err(InitError::Write)?;
            ace.done("Created .gitignore");
        }

        PullImports { school_root: self.project_dir }.run(ace)?;

        Ok(())
    }
}

fn ensure_standard_import(toml: &mut school_toml::SchoolToml) {
    let already = toml.imports.iter().any(|i|
        i.source == STANDARD_SCHOOL_SOURCE && i.skill == "*"
    );
    if !already {
        toml.imports.push(ImportDecl {
            skill: "*".to_string(),
            source: STANDARD_SCHOOL_SOURCE.to_string(),
            include_experimental: false,
            include_system: false,
        });
    }
}
