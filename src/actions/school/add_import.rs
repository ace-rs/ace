use std::path::Path;

use crate::ace::Ace;
use crate::config;
use crate::config::school_toml::ImportDecl;

use crate::skills::discover::{DiscoveredSkill, discover_skills};

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
}

/// Result of a successful import — or a request for the caller to pick a skill.
pub enum AddImportResult {
    Done {
        #[allow(dead_code)] // part of result API
        skill: String,
    },
    NeedsSelection(Vec<DiscoveredSkill>),
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

        let skills = discover_skills(&cached)?;
        if skills.is_empty() {
            return Err(AddImportError::NoSkills(self.source.to_string()));
        }

        let selected = match self.skill {
            Some(name) => skills
                .iter()
                .find(|s| s.id == name)
                .ok_or_else(|| AddImportError::SkillNotFound(name.to_string()))?,
            None if skills.len() == 1 => &skills[0],
            None => return Ok(AddImportResult::NeedsSelection(skills)),
        };

        if !warn_if_rejected(selected, ace) {
            return Err(AddImportError::RejectedImports { count: 1 });
        }
        self.install_skill(selected)?;

        ace.done(&format!("Imported skill: {}", selected.id));
        Ok(AddImportResult::Done {
            skill: selected.id.to_string(),
        })
    }

    /// Install a specific discovered skill after selection.
    pub fn install_selected(
        &self,
        skill: &DiscoveredSkill,
        ace: &mut Ace,
    ) -> Result<(), AddImportError> {
        ace.progress(&format!("Installing {}", skill.id));
        if !warn_if_rejected(skill, ace) {
            return Err(AddImportError::RejectedImports { count: 1 });
        }
        self.install_skill(skill)?;
        ace.done(&format!("Imported skill: {}", skill.id));
        Ok(())
    }

    fn install_skill(&self, skill: &DiscoveredSkill) -> Result<(), AddImportError> {
        let dest = self.school_root.join("skills").join(skill.id.as_str());
        if dest.exists() {
            std::fs::remove_dir_all(&dest)?;
        }

        crate::fsutil::copy_dir_recursive(&skill.path, &dest)?;

        let toml_path = self.school_root.join("school.toml");
        let mut school = config::school_toml::load(&toml_path)?;

        // Match against the canonical plural set, not just the singular
        // alias — a decl using the new `skills = [...]` form must still
        // be detected as a duplicate for the same skill.
        let needle = skill.id.as_str();
        let entry = school
            .imports
            .iter_mut()
            .find(|i| i.patterns().contains(&needle));
        match entry {
            Some(existing) => existing.source = self.source.to_string(),
            None => school.imports.push(ImportDecl {
                source: self.source.to_string(),
                skills: vec![skill.id.to_string()],
                ..ImportDecl::default()
            }),
        }

        config::school_toml::save(&toml_path, &school)?;
        Ok(())
    }
}

fn warn_if_rejected(skill: &DiscoveredSkill, ace: &mut Ace) -> bool {
    if let Err(reason) = skill.admission() {
        ace.warn(&format!(
            "skipping inadmissible skill `{}`: {reason}",
            crate::skills::name::render(skill.id.as_str()),
        ));
        return false;
    }
    if let Some(warning) = skill.frontmatter_warning() {
        ace.warn(&warning);
        ace.hint(crate::skills::discover::FRONTMATTER_WARNING_HINT);
    }
    true
}
