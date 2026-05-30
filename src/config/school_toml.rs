use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::config::ace_toml::BackendDecl;
use super::{is_empty_str, is_empty_map, is_empty_vec, is_false, ConfigError};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct SchoolToml {
    pub name: String,
    /// Default backend name (built-in or one declared in `backends` below).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(skip_serializing_if = "is_empty_str")]
    pub session_prompt: String,
    #[serde(skip_serializing_if = "is_empty_map")]
    pub env: HashMap<String, String>,
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub mcp: Vec<McpDecl>,
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub projects: Vec<Project>,
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub imports: Vec<ImportDecl>,
    /// Custom backend declarations seeded by the school. Layered upstream of
    /// user/project/local `[[backends]]` decls.
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub backends: Vec<BackendDecl>,
}

/// One `[[imports]]` declaration. Spec: `docs/spec/skills/selection.md`
/// § `[[imports]]` schema.
///
/// Two pattern fields coexist for backcompat:
/// - `skills: Vec<String>` is the canonical plural form — the only form
///   ever emitted.
/// - `skill: String` is the historical singular alias. Liberally accepted
///   on deserialize, then folded into `skills` by [`ImportDecl::normalize`]
///   (called from [`load`]) and never re-emitted. Interior code only ever
///   sees the plural form.
///
/// At least one of the two must be non-empty for the decl to select
/// anything.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ImportDecl {
    pub source: String,
    /// Canonical plural form. Empty by default.
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub skills: Vec<String>,
    /// Backcompat singular alias. Accepted on load, normalized into
    /// `skills`, never emitted.
    #[serde(skip_serializing)]
    pub skill: String,
    /// Patterns to subtract from the matched set. Also doubles as the
    /// collision-warning suppressor — when a sibling import would collide,
    /// listing the offending pattern here drops the warning.
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub exclude_skills: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    pub include_experimental: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub include_system: bool,
    /// Admit `internal: true` skills via glob matches in `skills`.
    /// Explicit-name patterns bypass the internal filter regardless
    /// (mirrors skills.sh).
    #[serde(skip_serializing_if = "is_false")]
    pub include_internal: bool,
}

impl ImportDecl {
    /// Fold the singular `skill` alias into the canonical `skills` list
    /// (skills-first order, deduped) and clear it. Called from [`load`] so
    /// interior code only ever sees the plural form.
    fn normalize(&mut self) {
        let alias = std::mem::take(&mut self.skill);
        if !alias.is_empty() && !self.skills.iter().any(|s| s == &alias) {
            self.skills.push(alias);
        }
    }

    /// Match handles selecting which skills to import.
    pub fn patterns(&self) -> Vec<&str> {
        self.skills.iter().map(String::as_str).collect()
    }

    /// True when the decl selects no skills at all (caller must error).
    pub fn has_patterns(&self) -> bool {
        !self.skills.is_empty()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct McpDecl {
    pub name: String,
    pub url: String,
    #[serde(skip_serializing_if = "is_empty_map")]
    pub headers: HashMap<String, String>,
    #[serde(skip_serializing_if = "is_empty_str")]
    pub instructions: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Project {
    pub name: String,
    pub repo: String,
    #[serde(skip_serializing_if = "is_empty_str")]
    pub description: String,
    #[serde(skip_serializing_if = "is_empty_map")]
    pub env: HashMap<String, String>,
}

pub fn load(path: &Path) -> Result<SchoolToml, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let mut config: SchoolToml = toml::from_str(&content)?;
    for decl in &mut config.imports {
        decl.normalize();
    }
    Ok(config)
}

pub fn save(path: &Path, toml: &SchoolToml) -> Result<(), ConfigError> {
    let content = toml::to_string_pretty(toml)?;
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- backcompat: singular `skill` --

    #[test]
    fn import_decl_default_flags_false() {
        let toml_str = "skill = \"foo\"\nsource = \"owner/repo\"\n";
        let decl: ImportDecl = toml::from_str(toml_str).expect("parse");
        assert_eq!(decl.skill, "foo");
        assert_eq!(decl.source, "owner/repo");
        assert!(!decl.include_experimental);
        assert!(!decl.include_system);
        assert!(!decl.include_internal);
        assert!(decl.exclude_skills.is_empty());
        assert!(decl.skills.is_empty());
    }

    #[test]
    fn import_decl_parses_include_flags() {
        let toml_str = "skill = \"*\"\nsource = \"owner/repo\"\ninclude_experimental = true\ninclude_system = true\n";
        let decl: ImportDecl = toml::from_str(toml_str).expect("parse");
        assert!(decl.include_experimental);
        assert!(decl.include_system);
    }

    #[test]
    fn import_decl_omits_false_flags_when_serialized() {
        let decl = ImportDecl {
            skills: vec!["foo".to_string()],
            source: "owner/repo".to_string(),
            ..ImportDecl::default()
        };
        let out = toml::to_string(&decl).expect("serialize");
        assert!(!out.contains("include_experimental"),
            "false include_experimental should not be serialized: {out}");
        assert!(!out.contains("include_system"),
            "false include_system should not be serialized: {out}");
        assert!(!out.contains("include_internal"),
            "false include_internal should not be serialized: {out}");
    }

    #[test]
    fn import_decl_writes_true_flags() {
        let decl = ImportDecl {
            skills: vec!["*".to_string()],
            source: "owner/repo".to_string(),
            include_experimental: true,
            ..ImportDecl::default()
        };
        let out = toml::to_string(&decl).expect("serialize");
        assert!(out.contains("include_experimental = true"), "missing flag in {out}");
        assert!(!out.contains("include_system"), "false flag should be omitted: {out}");
    }

    // -- plural `skills` (canonical form) --

    #[test]
    fn import_decl_parses_skills_array() {
        let toml_str = "source = \"owner/repo\"\nskills = [\"alpha\", \"beta\"]\n";
        let decl: ImportDecl = toml::from_str(toml_str).expect("parse");
        assert_eq!(decl.skills, vec!["alpha".to_string(), "beta".to_string()]);
        assert!(decl.skill.is_empty());
        assert_eq!(decl.patterns(), vec!["alpha", "beta"]);
    }

    #[test]
    fn import_decl_writers_emit_plural_skills() {
        let decl = ImportDecl {
            source: "owner/repo".to_string(),
            skills: vec!["alpha".to_string(), "beta".to_string()],
            ..ImportDecl::default()
        };
        let out = toml::to_string(&decl).expect("serialize");
        assert!(out.contains("skills = [\"alpha\", \"beta\"]"), "missing plural form: {out}");
        assert!(!out.contains("skill = \""), "should not emit singular alias when plural is canonical: {out}");
    }

    #[test]
    fn import_decl_normalize_folds_singular_into_skills() {
        let mut decl = ImportDecl {
            source: "owner/repo".to_string(),
            skill: "foo".to_string(),
            ..ImportDecl::default()
        };
        decl.normalize();
        assert_eq!(decl.skills, vec!["foo".to_string()]);
        assert!(decl.skill.is_empty(), "singular alias cleared after normalize");
    }

    #[test]
    fn import_decl_normalize_appends_and_dedups() {
        // skills-first order; singular appended only when absent.
        let mut append = ImportDecl {
            source: "owner/repo".to_string(),
            skills: vec!["bar".to_string()],
            skill: "foo".to_string(),
            ..ImportDecl::default()
        };
        append.normalize();
        assert_eq!(append.skills, vec!["bar".to_string(), "foo".to_string()]);
        assert!(append.skill.is_empty());

        let mut dup = ImportDecl {
            source: "owner/repo".to_string(),
            skills: vec!["foo".to_string(), "bar".to_string()],
            skill: "foo".to_string(),
            ..ImportDecl::default()
        };
        dup.normalize();
        assert_eq!(dup.skills, vec!["foo".to_string(), "bar".to_string()]);
    }

    #[test]
    fn import_decl_singular_normalized_emits_plural() {
        // Old schools written with the singular alias rewrite to the plural
        // form once loaded+normalized — the legacy key is never re-emitted
        // (pre-1.0 backcompat: normalize on load, drop alias on save).
        let mut decl: ImportDecl =
            toml::from_str("source = \"owner/repo\"\nskill = \"foo\"\n").expect("parse");
        decl.normalize();
        let out = toml::to_string(&decl).expect("serialize");
        assert!(out.contains("skills = [\"foo\"]"), "expected plural form: {out}");
        assert!(!out.contains("skill = "), "singular alias must not be emitted: {out}");
    }

    #[test]
    fn import_decl_has_patterns_reflects_skills() {
        let empty = ImportDecl {
            source: "owner/repo".to_string(),
            ..ImportDecl::default()
        };
        assert!(!empty.has_patterns());

        let plural = ImportDecl {
            source: "owner/repo".to_string(),
            skills: vec!["foo".to_string()],
            ..ImportDecl::default()
        };
        assert!(plural.has_patterns());
    }

    // -- exclude_skills + include_internal --

    #[test]
    fn import_decl_parses_exclude_skills() {
        let toml_str = "source = \"owner/repo\"\nskills = [\"*\"]\nexclude_skills = [\"rust-coding\"]\n";
        let decl: ImportDecl = toml::from_str(toml_str).expect("parse");
        assert_eq!(decl.exclude_skills, vec!["rust-coding".to_string()]);
    }

    #[test]
    fn import_decl_parses_include_internal() {
        let toml_str = "source = \"owner/repo\"\nskills = [\"*\"]\ninclude_internal = true\n";
        let decl: ImportDecl = toml::from_str(toml_str).expect("parse");
        assert!(decl.include_internal);
    }

    #[test]
    fn import_decl_exclude_skills_empty_omitted_on_save() {
        let decl = ImportDecl {
            source: "owner/repo".to_string(),
            skills: vec!["*".to_string()],
            ..ImportDecl::default()
        };
        let out = toml::to_string(&decl).expect("serialize");
        assert!(!out.contains("exclude_skills"), "empty exclude should be omitted: {out}");
    }
}
