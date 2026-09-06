//! Edit the per-scope skills selection fields in `ace.toml`.
//!
//! Pure-logic `apply()` mutates an `AceToml` in place; the `EditSkillsConfig`
//! action delegates targeted document edits for the chosen scope. Per-scope dedup is
//! automatic since each scope owns its own `AceToml` file.

use std::path::PathBuf;

use super::edit_config::{EditConfig, FieldEdit};
use crate::ace::Ace;
use crate::config::ace_toml::{self, AceToml};
use crate::config::{ConfigError, Scope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    /// Append patterns to `include_skills`. Silent no-op on duplicates within scope.
    Include(Vec<String>),
    /// Append patterns to `exclude_skills`. Silent no-op on duplicates within scope.
    Exclude(Vec<String>),
    /// Clear one or both selection lists.
    Reset(ResetTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetTarget {
    Include,
    Exclude,
    Both,
}

pub struct EditSkillsConfig {
    pub scope: Scope,
    pub op: Op,
}

impl EditSkillsConfig {
    pub fn run(&self, ace: &mut Ace) -> Result<(), ConfigError> {
        let paths = ace.paths();
        let path: PathBuf = self.scope.path_in(paths).to_path_buf();
        let mut toml = ace_toml::load_or_default(&path)?;
        apply(&mut toml, &self.op);
        let assignments = match &self.op {
            Op::Include(_) => {
                vec![FieldEdit::strings("include_skills", &toml.include_skills)]
            }
            Op::Exclude(_) => {
                vec![FieldEdit::strings("exclude_skills", &toml.exclude_skills)]
            }
            Op::Reset(ResetTarget::Include) => vec![FieldEdit::remove("include_skills")],
            Op::Reset(ResetTarget::Exclude) => vec![FieldEdit::remove("exclude_skills")],
            Op::Reset(ResetTarget::Both) => vec![
                FieldEdit::remove("include_skills"),
                FieldEdit::remove("exclude_skills"),
            ],
        };

        EditConfig {
            path: &path,
            assignments,
        }
        .run(ace)
    }
}

/// Mutate `toml` per `op`. Pure — no I/O.
pub fn apply(toml: &mut AceToml, op: &Op) {
    match op {
        Op::Include(patterns) => append_dedup(&mut toml.include_skills, patterns),
        Op::Exclude(patterns) => append_dedup(&mut toml.exclude_skills, patterns),
        Op::Reset(ResetTarget::Include) => toml.include_skills.clear(),
        Op::Reset(ResetTarget::Exclude) => toml.exclude_skills.clear(),
        Op::Reset(ResetTarget::Both) => {
            toml.include_skills.clear();
            toml.exclude_skills.clear();
        }
    }
}

fn append_dedup(target: &mut Vec<String>, patterns: &[String]) {
    for pat in patterns {
        if !target.iter().any(|p| p == pat) {
            target.push(pat.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vs(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn include_appends_to_empty_list() {
        let mut t = AceToml::default();
        apply(&mut t, &Op::Include(vs(&["rust-*"])));
        assert_eq!(t.include_skills, vs(&["rust-*"]));
        assert!(t.exclude_skills.is_empty());
        assert!(t.skills.is_empty());
    }

    #[test]
    fn include_appends_multiple_patterns() {
        let mut t = AceToml::default();
        apply(&mut t, &Op::Include(vs(&["rust-*", "issue-tracker"])));
        assert_eq!(t.include_skills, vs(&["rust-*", "issue-tracker"]));
    }

    #[test]
    fn include_dedups_within_scope() {
        let mut t = AceToml {
            include_skills: vs(&["rust-*"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Include(vs(&["rust-*", "issue-tracker"])));
        assert_eq!(t.include_skills, vs(&["rust-*", "issue-tracker"]));
    }

    #[test]
    fn include_preserves_existing_order() {
        let mut t = AceToml {
            include_skills: vs(&["a", "b"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Include(vs(&["c"])));
        assert_eq!(t.include_skills, vs(&["a", "b", "c"]));
    }

    #[test]
    fn include_does_not_touch_exclude_or_skills() {
        let mut t = AceToml {
            skills: vs(&["only-this"]),
            exclude_skills: vs(&["bad"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Include(vs(&["new"])));
        assert_eq!(t.skills, vs(&["only-this"]));
        assert_eq!(t.exclude_skills, vs(&["bad"]));
        assert_eq!(t.include_skills, vs(&["new"]));
    }

    #[test]
    fn exclude_appends_and_dedups() {
        let mut t = AceToml {
            exclude_skills: vs(&["foo"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Exclude(vs(&["foo", "bar"])));
        assert_eq!(t.exclude_skills, vs(&["foo", "bar"]));
    }

    #[test]
    fn exclude_does_not_touch_include_or_skills() {
        let mut t = AceToml {
            skills: vs(&["only-this"]),
            include_skills: vs(&["nice"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Exclude(vs(&["bad"])));
        assert_eq!(t.skills, vs(&["only-this"]));
        assert_eq!(t.include_skills, vs(&["nice"]));
        assert_eq!(t.exclude_skills, vs(&["bad"]));
    }

    #[test]
    fn reset_include_only_drops_include() {
        let mut t = AceToml {
            include_skills: vs(&["a", "b"]),
            exclude_skills: vs(&["c"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Reset(ResetTarget::Include));
        assert!(t.include_skills.is_empty());
        assert_eq!(t.exclude_skills, vs(&["c"]));
    }

    #[test]
    fn reset_exclude_only_drops_exclude() {
        let mut t = AceToml {
            include_skills: vs(&["a"]),
            exclude_skills: vs(&["b", "c"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Reset(ResetTarget::Exclude));
        assert_eq!(t.include_skills, vs(&["a"]));
        assert!(t.exclude_skills.is_empty());
    }

    #[test]
    fn reset_both_drops_both_lists() {
        let mut t = AceToml {
            include_skills: vs(&["a"]),
            exclude_skills: vs(&["b"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Reset(ResetTarget::Both));
        assert!(t.include_skills.is_empty());
        assert!(t.exclude_skills.is_empty());
    }

    #[test]
    fn reset_never_touches_skills_field() {
        let mut t = AceToml {
            skills: vs(&["allowlist"]),
            include_skills: vs(&["a"]),
            exclude_skills: vs(&["b"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Reset(ResetTarget::Both));
        assert_eq!(t.skills, vs(&["allowlist"]));
    }

    #[test]
    fn include_empty_list_is_noop() {
        let mut t = AceToml {
            include_skills: vs(&["existing"]),
            ..AceToml::default()
        };
        apply(&mut t, &Op::Include(vs(&[])));
        assert_eq!(t.include_skills, vs(&["existing"]));
    }
}
