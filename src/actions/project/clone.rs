use std::path::Path;

use crate::ace::Ace;
use crate::actions::project::PrepareError;
use crate::config;
use crate::config::index_toml;
use crate::git;

/// Install or reinstall school: git clone + index update. Also used as the
/// self-heal path when a prior clone is missing or partial.
pub struct Clone<'a> {
    pub project_dir: &'a Path,
    pub specifier: &'a str,
}

impl Clone<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), PrepareError> {
        let school_paths = config::school_paths::resolve(self.project_dir, self.specifier)?;
        let Some(clone_path) = &school_paths.clone_path else {
            return Ok(()); // embedded school
        };

        if let Some(parent) = clone_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PrepareError::Clone(format!("mkdir: {e}")))?;
        }

        // Partial clone dir (no .git) left behind by a prior aborted install or
        // pre-XDG migration — remove it so git clone has a clean target.
        if clone_path.exists() && !clone_path.join(".git").exists() {
            std::fs::remove_dir_all(clone_path)
                .map_err(|e| PrepareError::Clone(format!("remove stale clone dir: {e}")))?;
        }

        let raw_repo = self
            .specifier
            .split_once(':')
            .map_or(self.specifier, |(owner_repo, _)| owner_repo);
        let repo = git::normalize_github_source(raw_repo);
        let url = format!("https://github.com/{repo}.git");

        ace.progress(&format!("Cloning {repo}"));
        if let Err(e) = git::clone_repo(&url, clone_path) {
            ace.warn(&e.to_string());
            ace.hint(git::auth_hint());
            return Err(PrepareError::Clone(e.to_string()));
        }
        ace.done(&format!("Cloned {repo}"));

        update_index(&school_paths.source)?;

        let school_toml_path = school_paths.root.join("school.toml");
        let school_toml = config::school_toml::load(&school_toml_path)?;
        ace.done(&format!("School: {}", school_toml.name));

        Ok(())
    }
}

fn update_index(source: &str) -> Result<(), PrepareError> {
    let index_path =
        index_toml::index_path().map_err(|e| PrepareError::Clone(format!("index path: {e}")))?;
    let legacy_path = index_toml::legacy_index_path()
        .map_err(|e| PrepareError::Clone(format!("legacy index path: {e}")))?;
    let mut index = index_toml::load_or_migrate(&index_path, &legacy_path)
        .map_err(|e| PrepareError::Clone(format!("load index: {e}")))?;
    index_toml::upsert(&mut index, source);
    index_toml::save(&index_path, &index)
        .map_err(|e| PrepareError::Clone(format!("save index: {e}")))?;
    Ok(())
}
