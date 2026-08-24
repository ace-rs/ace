use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::str::FromStr;

use super::{ConfigError, is_empty_map, is_empty_str};

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct BackendDecl {
    #[serde(skip)]
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
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
    /// Backend name (resolved against the registry — built-ins or `[backends.<name>]`
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

    /// Per-backend declarations keyed by their registry identity.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub backends: BTreeMap<String, BackendDecl>,
}

pub fn load(path: &Path) -> Result<AceToml, ConfigError> {
    let content = std::fs::read_to_string(path)?;
    let mut config: AceToml = toml::from_str(&content)?;
    inject_backend_names(&mut config.backends);
    Ok(config)
}

/// Load from file, returning default if the file doesn't exist.
/// Errors on invalid TOML or other I/O failures.
pub fn load_or_default(path: &Path) -> Result<AceToml, ConfigError> {
    match std::fs::read_to_string(path) {
        Ok(content) => {
            let mut config: AceToml = toml::from_str(&content)?;
            inject_backend_names(&mut config.backends);
            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AceToml::default()),
        Err(e) => Err(ConfigError::from(e)),
    }
}

fn inject_backend_names(backends: &mut BTreeMap<String, BackendDecl>) {
    for (name, backend) in backends {
        backend.name.clone_from(name);
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
    fn load_parses_keyed_backends() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(
            &path,
            "[backends.claude]\nmodel = \"opus\"\neffort = \"high\"\n\n[backends.claude.env]\nANTHROPIC_BASE_URL = \"https://example.com\"\n",
        )
        .expect("write");

        let config = load(&path).expect("keyed backend table must parse");
        let backend = config.backends.get("claude").expect("claude backend");
        assert_eq!(backend.name, "claude");
        assert_eq!(backend.model.as_deref(), Some("opus"));
        assert_eq!(backend.effort.as_deref(), Some("high"));
        assert_eq!(
            backend.env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://example.com"),
        );
    }

    #[test]
    fn load_rejects_legacy_backends_array() {
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

        assert!(
            load(&path).is_err(),
            "legacy backend arrays must be rejected"
        );
    }

    #[test]
    fn exclude_mcp_round_trip() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "exclude_mcp = [\"github\", \"linear\"]\n").expect("write");

        let loaded = load_or_default(&path).expect("load");
        assert_eq!(
            loaded.exclude_mcp,
            vec!["github".to_string(), "linear".to_string()]
        );

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
