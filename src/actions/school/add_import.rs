use std::path::Path;

use crate::ace::Ace;
use crate::config;
use crate::config::school_toml::ImportDecl;

use crate::skills::discover::discover_skills;
use crate::skills::{Discovered, FRONTMATTER_WARNING_HINT, Skill, name};

pub struct AddImport<'a> {
    pub source: &'a str,
    pub skill: Option<&'a str>,
    pub school_root: &'a Path,
}

#[derive(Debug, thiserror::Error)]
pub enum AddImportError {
    #[error("{0}")]
    Clone(#[from] crate::git::GitError),
    #[error("no skills found in {0}")]
    NoSkills(String),
    #[error("skill not found: {0}")]
    SkillNotFound(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(#[from] config::ConfigError),
    #[error("skipped {count} inadmissible skill(s)")]
    RejectedImports { count: usize },
    #[error("skill `{0}` is committed as a broken git submodule")]
    BrokenSubmodule(String),
}

/// Result of a successful import — or a request for the caller to pick a skill.
pub enum AddImportResult {
    Done,
    NeedsSelection(Vec<Skill<Discovered>>),
}

impl AddImport<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<AddImportResult, AddImportError> {
        ace.progress(&format!("Fetching {}", self.source));
        let cached = match crate::git::ensure_source_cache(self.source) {
            Ok(p) => p,
            Err(e) => {
                ace.warn(&e.to_string());
                ace.hint(crate::git::auth_hint());
                return Err(e.into());
            }
        };

        let (skills, prunes) = discover_skills(&cached)?;
        for reason in &prunes {
            ace.warn(&format!("skipping malformed skill identity: {reason}"));
        }
        if skills.is_empty() {
            return Err(AddImportError::NoSkills(self.source.to_string()));
        }

        let selected = match self.skill {
            Some(name) => skills
                .iter()
                .find(|s| s.locator == name)
                .ok_or_else(|| AddImportError::SkillNotFound(name.to_string()))?,
            None if skills.len() == 1 => &skills[0],
            None => return Ok(AddImportResult::NeedsSelection(skills)),
        };

        if !warn_if_rejected(selected, ace) {
            return Err(AddImportError::RejectedImports { count: 1 });
        }
        self.install_skill(selected, ace)?;

        ace.done(&format!("Imported skill: {}", selected.locator));
        Ok(AddImportResult::Done)
    }

    /// Install a specific discovered skill after selection.
    pub fn install_selected(
        &self,
        skill: &Skill<Discovered>,
        ace: &mut Ace,
    ) -> Result<(), AddImportError> {
        ace.progress(&format!("Installing {}", skill.locator));
        if !warn_if_rejected(skill, ace) {
            return Err(AddImportError::RejectedImports { count: 1 });
        }
        self.install_skill(skill, ace)?;
        ace.done(&format!("Imported skill: {}", skill.locator));
        Ok(())
    }

    fn install_skill(
        &self,
        skill: &Skill<Discovered>,
        ace: &mut Ace,
    ) -> Result<(), AddImportError> {
        let name = skill.locator.as_str();

        // Refuse to overwrite a path the host repo tracks as a gitlink — an
        // earlier import that leaked a `.git` turned it into an accidental
        // submodule. Writing files there leaves a confusing half-state; warn
        // and bail so the user clears the index entry first.
        let names = [name.to_string()];
        if !super::gitlink::gitlinked_names(self.school_root, &names).is_empty() {
            super::gitlink::warn_broken_submodule(ace, name);
            return Err(AddImportError::BrokenSubmodule(name.to_string()));
        }

        let dest = self.school_root.join("skills").join(name);
        crate::fsutil::replace_dir_recursive(&skill.path, &dest)?;

        let toml_path = self.school_root.join("school.toml");
        let mut school = config::school_toml::load(&toml_path)?;

        // Match against the canonical plural set, not just the singular
        // alias — a decl using the new `skills = [...]` form must still
        // be detected as a duplicate for the same skill.
        let needle = skill.locator.as_str();
        let entry = school
            .imports
            .iter_mut()
            .find(|i| i.patterns().contains(&needle));
        match entry {
            Some(existing) => existing.source = self.source.to_string(),
            None => school.imports.push(ImportDecl {
                source: self.source.to_string(),
                skills: vec![skill.locator.as_str().to_string()],
                ..ImportDecl::default()
            }),
        }

        config::school_toml::save(&toml_path, &school)?;
        Ok(())
    }
}

fn warn_if_rejected(skill: &Skill<Discovered>, ace: &mut Ace) -> bool {
    if let Err(reason) = skill.admission() {
        ace.warn(&format!(
            "skipping inadmissible skill `{}`: {reason}",
            name::render(skill.locator.as_str()),
        ));
        return false;
    }
    if let Some(warning) = skill.frontmatter_warning() {
        ace.warn(&warning);
        ace.hint(FRONTMATTER_WARNING_HINT);
    }
    true
}
