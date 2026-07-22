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
        merge_import(&mut school.imports, self.source, skill.locator.as_str());

        config::school_toml::save(&toml_path, &school)?;
        Ok(())
    }
}

/// Record `name` under `source`, reusing the decl that already covers it.
///
/// Three cases, in order: the skill is already declared somewhere (re-point that
/// decl at the new source), a literal decl for this source exists (fold the name
/// into its `skills`), or neither (append a decl). Without the middle case every
/// import appends a fresh `[[imports]]` block for a source the file already
/// lists — which a multi-skill import would do once per pick.
fn merge_import(imports: &mut Vec<ImportDecl>, source: &str, name: &str) {
    if let Some(existing) = imports.iter_mut().find(|i| i.patterns().contains(&name)) {
        existing.source = source.to_string();
        return;
    }

    // Glob decls get their own entry rather than absorbing the name. An
    // explicit name bypasses the tier filter a pattern is subject to, so the
    // literal decl carries meaning the glob cannot express.
    let literal = imports
        .iter_mut()
        .find(|i| i.source == source && !i.patterns().iter().any(|p| crate::glob::is_glob(p)));

    match literal {
        Some(decl) => decl.skills.push(name.to_string()),
        None => imports.push(ImportDecl {
            source: source.to_string(),
            skills: vec![name.to_string()],
            ..ImportDecl::default()
        }),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn decl(source: &str, skills: &[&str]) -> ImportDecl {
        ImportDecl {
            source: source.to_string(),
            skills: skills.iter().map(|s| s.to_string()).collect(),
            ..ImportDecl::default()
        }
    }

    #[test]
    fn merge_appends_decl_when_source_is_new() {
        let mut imports = vec![decl("gh:other/repo", &["alpha"])];
        merge_import(&mut imports, "gh:acme/skills", "beta");

        assert_eq!(imports.len(), 2);
        assert_eq!(imports[1].skills, vec!["beta"]);
    }

    #[test]
    fn merge_folds_into_existing_decl_for_same_source() {
        let mut imports = vec![decl("gh:acme/skills", &["alpha"])];
        merge_import(&mut imports, "gh:acme/skills", "beta");

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].skills, vec!["alpha", "beta"]);
    }

    #[test]
    fn merge_repoints_source_when_skill_already_declared() {
        let mut imports = vec![decl("gh:old/repo", &["alpha"])];
        merge_import(&mut imports, "gh:new/repo", "alpha");

        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].source, "gh:new/repo");
        assert_eq!(imports[0].skills, vec!["alpha"]);
    }

    #[test]
    fn merge_leaves_glob_decl_alone() {
        let mut imports = vec![decl("gh:acme/skills", &["*"])];
        merge_import(&mut imports, "gh:acme/skills", "beta");

        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].skills, vec!["*"]);
        assert_eq!(imports[1].skills, vec!["beta"]);
    }
}
