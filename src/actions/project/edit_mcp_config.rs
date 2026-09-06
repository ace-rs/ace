//! Edit personal MCP exclusions without rewriting unrelated configuration.

use std::path::Path;

use crate::ace::Ace;
use crate::config::ConfigError;
use crate::config::ace_toml::{self, AceToml};

use super::edit_config::{EditConfig, FieldEdit};

pub enum Op {
    Include(String),
    Exclude(Vec<String>),
}

pub struct EditMcpConfig<'a> {
    pub path: &'a Path,
    pub op: Op,
}

impl EditMcpConfig<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), ConfigError> {
        let mut toml = ace_toml::load_or_default(self.path)?;
        apply(&mut toml, &self.op);
        let edit = if toml.exclude_mcp.is_empty() {
            FieldEdit::remove("exclude_mcp")
        } else {
            FieldEdit::strings("exclude_mcp", &toml.exclude_mcp)
        };

        EditConfig {
            path: self.path,
            assignments: vec![edit],
        }
        .run(ace)
    }
}

fn apply(toml: &mut AceToml, op: &Op) {
    match op {
        Op::Include(name) => toml.exclude_mcp.retain(|existing| existing != name),
        Op::Exclude(names) => {
            for name in names {
                if !toml.exclude_mcp.contains(name) {
                    toml.exclude_mcp.push(name.clone());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_appends_to_empty() {
        let mut toml = AceToml::default();

        apply(&mut toml, &Op::Exclude(vec!["github".into()]));

        assert_eq!(toml.exclude_mcp, vec!["github".to_string()]);
    }

    #[test]
    fn exclude_dedups_on_double_add() {
        let mut toml = AceToml::default();

        apply(&mut toml, &Op::Exclude(vec!["github".into()]));
        apply(&mut toml, &Op::Exclude(vec!["github".into()]));

        assert_eq!(toml.exclude_mcp, vec!["github".to_string()]);
    }

    #[test]
    fn exclude_appends_multiple_distinct() {
        let mut toml = AceToml::default();

        apply(
            &mut toml,
            &Op::Exclude(vec!["github".into(), "linear".into()]),
        );

        assert_eq!(
            toml.exclude_mcp,
            vec!["github".to_string(), "linear".to_string()]
        );
    }

    #[test]
    fn include_removes_existing() {
        let mut toml = AceToml {
            exclude_mcp: vec!["github".into(), "linear".into()],
            ..AceToml::default()
        };

        apply(&mut toml, &Op::Include("github".into()));

        assert_eq!(toml.exclude_mcp, vec!["linear".to_string()]);
    }

    #[test]
    fn include_idempotent_on_missing() {
        let mut toml = AceToml::default();

        apply(&mut toml, &Op::Include("ghost".into()));

        assert!(toml.exclude_mcp.is_empty());
    }

    #[test]
    fn exclude_preserves_other_fields() {
        let mut toml = AceToml {
            school: "foo/bar".into(),
            ..AceToml::default()
        };

        apply(&mut toml, &Op::Exclude(vec!["github".into()]));

        assert_eq!(toml.school, "foo/bar");
        assert_eq!(toml.exclude_mcp, vec!["github".to_string()]);
    }
}
