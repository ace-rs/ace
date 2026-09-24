use crate::ace::Ace;
use crate::actions::project::PrepareError;
use crate::config::index_toml;
use crate::git;
use crate::git::access::{self, Interaction};
use crate::school::linked::LinkedSchool;
use crate::school::toml as school_toml;

/// Install or reinstall school: git clone + index update. Also used as the
/// self-heal path when a prior clone is missing or partial.
pub struct CloneSchool<'a> {
    pub school: &'a LinkedSchool,
}

impl CloneSchool<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), PrepareError> {
        if self.school.clone_path.is_none() {
            return Ok(());
        }
        self.check_destination()?;
        let repo = git::normalize_source(&self.school.source);
        let ssh = format!("git@github.com:{repo}.git");
        let https = format!("https://github.com/{repo}.git");

        ace.progress(&format!("Checking access to {repo}"));
        match (access::Probe { url: &ssh }).run(ace.project_dir()) {
            Ok(()) => self.clone(ace, &ssh, Interaction::Unattended),
            Err(ssh_error) => match (access::Probe { url: &https }).run(ace.project_dir()) {
                Ok(()) => self.clone(ace, &https, Interaction::Unattended),
                Err(https_error) => Err(PrepareError::RepositoryAccess(format!(
                    "cannot access {repo}\nSSH: {ssh_error}\nHTTPS: {https_error}"
                ))),
            },
        }
    }

    /// Give Git the terminal for one HTTPS clone after the caller obtains consent.
    pub fn run_with_credentials(&self, ace: &mut Ace) -> Result<(), PrepareError> {
        self.check_destination()?;
        let repo = git::normalize_source(&self.school.source);
        let url = format!("https://github.com/{repo}.git");
        ace.info(&format!("Cloning {repo} with Git's credential prompts"));
        self.clone(ace, &url, Interaction::Attended)
    }

    fn check_destination(&self) -> Result<(), PrepareError> {
        if let Some(path) = &self.school.clone_path
            && path.exists()
        {
            return Err(PrepareError::Clone(format!(
                "school cache already exists at {}; inspect its contents before retrying",
                path.display()
            )));
        }
        Ok(())
    }

    fn clone(
        &self,
        ace: &mut Ace,
        url: &str,
        interaction: Interaction,
    ) -> Result<(), PrepareError> {
        let Some(clone_path) = &self.school.clone_path else {
            return Ok(()); // embedded school
        };

        if let Some(parent) = clone_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| PrepareError::Clone(format!("mkdir: {e}")))?;
        }

        let repo = git::normalize_source(&self.school.source);
        access::Clone {
            url,
            dest: clone_path,
            interaction,
        }
        .run(ace.project_dir())
        .map_err(|error| match error {
            git::GitError::Exit { status, .. } if crate::platform::exit_code(status) == 130 => {
                PrepareError::Cancelled
            }
            error => PrepareError::Clone(error.to_string()),
        })?;
        ace.done(&format!("Cloned {repo}"));

        let school_toml = school_toml::load(&self.school.toml_path())?;
        update_index(&self.school.raw_specifier)?;
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
