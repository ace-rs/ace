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
/// - `skills: Vec<String>` is the canonical plural form; writers always
///   emit this when populated.
/// - `skill: String` is the historical singular alias. Liberally accepted
///   on deserialize; round-tripped on serialize if non-empty.
///
/// At least one of the two must be non-empty for the decl to select
/// anything. Use [`ImportDecl::patterns`] to read the merged set.
#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct ImportDecl {
    pub source: String,
    /// Canonical plural form. Empty by default.
    #[serde(skip_serializing_if = "is_empty_vec")]
    pub skills: Vec<String>,
    /// Backcompat singular alias. Empty by default.
    #[serde(skip_serializing_if = "is_empty_str")]
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
    /// Union of `skills` (canonical) and `skill` (alias) in declaration
    /// order: `skills` entries first, then the singular alias if it
    /// wasn't already covered.
    pub fn patterns(&self) -> Vec<&str> {
        let mut out: Vec<&str> = self.skills.iter().map(String::as_str).collect();
        if !self.skill.is_empty() && !out.contains(&self.skill.as_str()) {
            out.push(self.skill.as_str());
        }
        out
    }

    /// True when the decl selects no skills at all (caller must error).
    #[allow(dead_code)] // used at imports-resolver slice
    pub fn has_patterns(&self) -> bool {
        !self.skills.is_empty() || !self.skill.is_empty()
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
    let config: SchoolToml = toml::from_str(&content)?;
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
            skill: "foo".to_string(),
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
            skill: "*".to_string(),
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
    fn import_decl_singular_alone_round_trips() {
        // Old schools written before the plural existed must continue to
        // round-trip the singular form (CLAUDE.md backcompat contract).
        let toml_str = "source = \"owner/repo\"\nskill = \"foo\"\n";
        let decl: ImportDecl = toml::from_str(toml_str).expect("parse");
        let out = toml::to_string(&decl).expect("serialize");
        assert!(out.contains("skill = \"foo\""), "singular form lost on round-trip: {out}");
        assert!(!out.contains("skills = "), "should not emit empty plural: {out}");
    }

    #[test]
    fn import_decl_patterns_merges_both_forms() {
        // Mixed-mode decl (both `skill` and `skills` set) is liberally
        // accepted. Patterns returns the union.
        let toml_str = "source = \"owner/repo\"\nskill = \"foo\"\nskills = [\"bar\", \"baz\"]\n";
        let decl: ImportDecl = toml::from_str(toml_str).expect("parse");
        assert_eq!(decl.patterns(), vec!["bar", "baz", "foo"]);
    }

    #[test]
    fn import_decl_patterns_dedups_when_singular_present_in_plural() {
        let decl = ImportDecl {
            source: "owner/repo".to_string(),
            skills: vec!["foo".to_string(), "bar".to_string()],
            skill: "foo".to_string(),
            ..ImportDecl::default()
        };
        assert_eq!(decl.patterns(), vec!["foo", "bar"]);
    }

    #[test]
    fn import_decl_has_patterns_reflects_either_field() {
        let empty = ImportDecl {
            source: "owner/repo".to_string(),
            ..ImportDecl::default()
        };
        assert!(!empty.has_patterns());

        let singular = ImportDecl {
            source: "owner/repo".to_string(),
            skill: "foo".to_string(),
            ..ImportDecl::default()
        };
        assert!(singular.has_patterns());

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
