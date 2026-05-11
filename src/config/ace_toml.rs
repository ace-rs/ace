use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;

use super::{is_empty_str, is_empty_map, ConfigError};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct BackendDecl {
    pub name: String,
    /// Explicit kind (built-in name: claude/codex/flaude). When omitted,
    /// kind is inferred from `name` matching a built-in, then from `cmd[0]`
    /// basename. See `backend::registry::resolve_kind`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Argv for launching the binary. Empty = default to `[kind.name()]`
    /// after resolution.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cmd: Vec<String>,
    #[serde(skip_serializing_if = "is_empty_map")]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Trust {
    #[default]
    Default,
    Auto,
    Yolo,
}

impl Trust {
    pub fn is_default(&self) -> bool {
        matches!(self, Trust::Default)
    }

    pub fn label(self) -> &'static str {
        match self {
            Trust::Default => "default",
            Trust::Auto => "auto",
            Trust::Yolo => "yolo",
        }
    }
}

impl FromStr for Trust {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "default" => Ok(Trust::Default),
            "auto" => Ok(Trust::Auto),
            "yolo" => Ok(Trust::Yolo),
            other => Err(format!(
                "invalid trust value `{other}` (expected: default, auto, yolo)"
            )),
        }
    }
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AceToml {
    #[serde(skip_serializing_if = "is_empty_str")]
    pub school: String,
    /// Backend name (resolved against the registry — built-ins or `[[backends]]`
    /// declarations). Stored as a string; validation happens at lookup time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    // TODO: add `role` and `description` fields so non-dev roles (e.g. PM) can
    // configure ace for requirements-only repos, docs/spec/ workflows, Jira/Trello sync, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_prompt: Option<String>,
    #[serde(skip_serializing_if = "is_empty_map")]
    pub env: HashMap<String, String>,
    #[serde(skip_serializing_if = "Trust::is_default")]
    pub trust: Trust,

    /// Auto-resume previous session. Personal-only (local config).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume: Option<bool>,

    /// Disable automatic version checks and background upgrades.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_update: Option<bool>,

    /// Deprecated: use `trust = "yolo"` instead. Kept for backcompat.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub yolo: bool,

    /// Per-project skill whitelist. Empty = all skills (base for resolution).
    /// Last-wins merge across scopes (local > project > user).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skills: Vec<String>,

    /// Always-add skill patterns. Union across all scopes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include_skills: Vec<String>,

    /// Always-remove skill patterns. Union across all scopes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude_skills: Vec<String>,

    /// MCP server names to skip during registration. Union across user/project/local scopes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude_mcp: Vec<String>,

    /// Per-backend declarations: env overrides for built-ins, full custom backends later.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub backends: Vec<BackendDecl>,
}

pub fn load(path: &Path) -> Result<AceToml, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let config: AceToml = toml::from_str(&content)?;
    Ok(config)
}

/// Load from file, returning default if the file doesn't exist.
/// Errors on invalid TOML or other I/O failures.
pub fn load_or_default(path: &Path) -> Result<AceToml, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(toml::from_str(&content)?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AceToml::default()),
        Err(e) => Err(ConfigError::from(e)),
    }
}

pub fn save(path: &Path, toml: &AceToml) -> Result<(), ConfigError> {
    let content = toml::to_string_pretty(toml)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Set the school specifier, preserving all other fields.
pub fn set_school(path: &Path, specifier: &str) -> Result<(), ConfigError> {
    let mut config = load_or_default(path)?;
    config.school = specifier.to_string();
    save(path, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_or_default_missing_file() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("missing").join("ace.toml");
        let result = load_or_default(&path).expect("should return default");
        assert!(result.school.is_empty());
        assert!(result.backend.is_none());
    }

    #[test]
    fn load_or_default_existing_file() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "school = \"ace-rs/school\"\n").expect("write");

        let result = load_or_default(&path).expect("should load");
        assert_eq!(result.school, "ace-rs/school");
    }

    #[test]
    fn load_or_default_invalid_toml() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "not valid {{{{ toml").expect("write");

        assert!(load_or_default(&path).is_err());
    }

    #[test]
    fn set_school_creates_new_file() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");

        set_school(&path, "ace-rs/school").expect("set school");

        let config = load(&path).expect("reload");
        assert_eq!(config.school, "ace-rs/school");
    }

    #[test]
    fn load_parses_backends_array() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(
            &path,
            "[[backends]]\nname = \"claude\"\n\n[backends.env]\nANTHROPIC_BASE_URL = \"https://example.com\"\n",
        )
        .expect("write");

        let config = load(&path).expect("load");
        assert_eq!(config.backends.len(), 1);
        assert_eq!(config.backends[0].name, "claude");
        assert!(config.backends[0].kind.is_none());
        assert!(config.backends[0].cmd.is_empty());
        assert_eq!(
            config.backends[0].env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://example.com"),
        );
    }

    #[test]
    fn load_parses_backends_with_kind_and_cmd() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(
            &path,
            r#"[[backends]]
name = "bedrock-claude"
kind = "claude"
cmd = ["claude-bedrock", "--profile", "prod"]

[backends.env]
AWS_REGION = "us-east-1"
"#,
        )
        .expect("write");

        let config = load(&path).expect("load");
        assert_eq!(config.backends.len(), 1);
        let b = &config.backends[0];
        assert_eq!(b.name, "bedrock-claude");
        assert_eq!(b.kind.as_deref(), Some("claude"));
        assert_eq!(b.cmd, vec!["claude-bedrock", "--profile", "prod"]);
        assert_eq!(b.env.get("AWS_REGION").map(String::as_str), Some("us-east-1"));
    }

    #[test]
    fn exclude_mcp_round_trip() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "exclude_mcp = [\"github\", \"linear\"]\n").expect("write");

        let loaded = load_or_default(&path).expect("load");
        assert_eq!(loaded.exclude_mcp, vec!["github".to_string(), "linear".to_string()]);

        save(&path, &loaded).expect("save");
        let text = std::fs::read_to_string(&path).expect("re-read");
        assert!(text.contains("exclude_mcp"), "missing key: {text}");
        assert!(text.contains("github"), "missing github: {text}");
        assert!(text.contains("linear"), "missing linear: {text}");
    }

    #[test]
    fn exclude_mcp_empty_omitted_on_save() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        let toml = AceToml::default();
        save(&path, &toml).expect("save");
        let text = std::fs::read_to_string(&path).expect("re-read");
        assert!(!text.contains("exclude_mcp"), "should be skipped: {text}");
    }

    #[test]
    fn set_school_preserves_env() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "school = \"old\"\n\n[env]\nKEY = \"value\"\n").expect("write");

        set_school(&path, "ace-rs/school").expect("set school");

        let config = load(&path).expect("reload");
        assert_eq!(config.school, "ace-rs/school");
        assert_eq!(config.env.get("KEY").map(String::as_str), Some("value"));
    }
}
