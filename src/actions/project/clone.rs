use crate::ace::Ace;
use crate::actions::project::PrepareError;
use crate::config::index_toml;
use crate::git;
use crate::school::linked::LinkedSchool;
use crate::school::toml as school_toml;

/// Install or reinstall school: git clone + index update. Also used as the
/// self-heal path when a prior clone is missing or partial.
pub struct Clone<'a> {
    pub school: &'a LinkedSchool,
}

impl Clone<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), PrepareError> {
        let Some(clone_path) = &self.school.clone_path else {
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

        let specifier = self.school.source.as_str();
        let raw_repo = specifier
            .split_once(':')
            .map_or(specifier, |(owner_repo, _)| owner_repo);
        let repo = git::normalize_source(raw_repo);
        let url = format!("https://github.com/{repo}.git");

        ace.progress(&format!("Cloning {repo}"));
        if let Err(e) = git::clone_repo(&url, clone_path) {
            ace.warn(&e.to_string());
            ace.hint(git::auth_hint());
            return Err(PrepareError::Clone(e.to_string()));
        }
        ace.done(&format!("Cloned {repo}"));

        update_index(&self.school.source)?;

        let school_toml = school_toml::load(&self.school.root.join("school.toml"))?;
        ace.done(&format!("School: {}", school_toml.name));

        Ok(())
    }
}

fn update_index(source: &str) -> Result<(), PrepareError> {
    let index_path =
        index_toml::index_path().map_err(|e| PrepareError::Clone(format!("index path: {e}")))?;
    let mut index = index_toml::load(&index_path)
        .map_err(|e| PrepareError::Clone(format!("load index: {e}")))?;
    index_toml::upsert(&mut index, source);
    index_toml::save(&index_path, &index)
        .map_err(|e| PrepareError::Clone(format!("save index: {e}")))?;
    Ok(())
}
