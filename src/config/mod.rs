pub mod ace_toml;
pub mod index_toml;
pub mod paths;
pub mod resolve;
pub mod tree;

use std::collections::HashMap;
use std::path::Path;

pub(crate) fn is_empty_str(s: &str) -> bool {
    s.is_empty()
}
pub(crate) fn is_empty_map(m: &HashMap<String, String>) -> bool {
    m.is_empty()
}
pub(crate) fn is_empty_vec<T>(v: &[T]) -> bool {
    v.is_empty()
}
pub(crate) fn is_false(b: &bool) -> bool {
    !*b
}

/// Config scope — determines which layer a write targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    User,
    Project,
    Local,
}

impl Scope {
    /// Default scope when no explicit flag is given, inferred from the key.
    /// Personal-only fields → Local, shared fields → Project.
    pub fn default_for_key(key: &str) -> Self {
        match key {
            "trust" | "resume" => Scope::Local,
            _ => Scope::Project,
        }
    }

    /// Resolve the filesystem path for this scope.
    pub fn path_in<'a>(&self, paths: &'a paths::AcePaths) -> &'a Path {
        match self {
            Scope::User => &paths.user,
            Scope::Project => &paths.project,
            Scope::Local => &paths.local,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::Project => "project",
            Scope::Local => "local",
        }
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;

    #[test]
    fn label_strings() {
        assert_eq!(Scope::User.label(), "user");
        assert_eq!(Scope::Project.label(), "project");
        assert_eq!(Scope::Local.label(), "local");
    }
}

/// Parsed config key for get/set operations.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigKey {
    School,
    Backend,
    Trust,
    Resume,
    SkipUpdate,
    SessionPrompt,
    Env(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum BackendConfigField {
    Model,
    Effort,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConfigSetKey {
    Readable(ConfigKey),
    Backend {
        name: String,
        field: BackendConfigField,
    },
}

impl ConfigSetKey {
    pub fn parse(key: &str) -> Option<Self> {
        if let Some(config_key) = ConfigKey::parse(key) {
            return Some(ConfigSetKey::Readable(config_key));
        }

        let backend_path = key.strip_prefix("backends.")?;
        let (name, field_name) = backend_path.rsplit_once('.')?;
        if name.is_empty() {
            return None;
        }

        let field = match field_name {
            "model" => BackendConfigField::Model,
            "effort" => BackendConfigField::Effort,
            _ => return None,
        };

        Some(ConfigSetKey::Backend {
            name: name.to_string(),
            field,
        })
    }

    pub fn scope_key(&self) -> &str {
        match self {
            ConfigSetKey::Readable(config_key) => config_key.scope_key(),
            ConfigSetKey::Backend { .. } => "backends",
        }
    }
}

impl ConfigKey {
    pub fn parse(key: &str) -> Option<Self> {
        if let Some(env_key) = key.strip_prefix("env.") {
            if env_key.is_empty() {
                return None;
            }
            return Some(ConfigKey::Env(env_key.to_string()));
        }

        match key {
            "school" => Some(ConfigKey::School),
            "backend" => Some(ConfigKey::Backend),
            "trust" => Some(ConfigKey::Trust),
            "resume" => Some(ConfigKey::Resume),
            "skip_update" => Some(ConfigKey::SkipUpdate),
            "session_prompt" => Some(ConfigKey::SessionPrompt),
            _ => None,
        }
    }

    pub fn scope_key(&self) -> &str {
        match self {
            ConfigKey::School => "school",
            ConfigKey::Backend => "backend",
            ConfigKey::Trust => "trust",
            ConfigKey::Resume => "resume",
            ConfigKey::SkipUpdate => "skip_update",
            ConfigKey::SessionPrompt => "session_prompt",
            ConfigKey::Env(_) => "env",
        }
    }
}

#[cfg(test)]
mod config_key_tests {
    use super::*;

    #[test]
    fn parse_skip_update() {
        assert_eq!(ConfigKey::parse("skip_update"), Some(ConfigKey::SkipUpdate));
    }

    #[test]
    fn skip_update_scope_key() {
        assert_eq!(ConfigKey::SkipUpdate.scope_key(), "skip_update");
    }

    #[test]
    fn skip_update_default_scope_is_project() {
        assert_eq!(Scope::default_for_key("skip_update"), Scope::Project);
    }
}

#[cfg(test)]
mod config_set_key_tests {
    use super::*;

    #[test]
    fn parse_backend_model() {
        assert_eq!(
            ConfigSetKey::parse("backends.claude.model"),
            Some(ConfigSetKey::Backend {
                name: "claude".to_string(),
                field: BackendConfigField::Model,
            }),
        );
    }

    #[test]
    fn parse_backend_effort_with_dotted_instance_name() {
        assert_eq!(
            ConfigSetKey::parse("backends.bedrock.claude.effort"),
            Some(ConfigSetKey::Backend {
                name: "bedrock.claude".to_string(),
                field: BackendConfigField::Effort,
            }),
        );
    }

    #[test]
    fn reject_unsupported_or_incomplete_backend_paths() {
        assert_eq!(ConfigSetKey::parse("backends.claude.cmd"), None);
        assert_eq!(ConfigSetKey::parse("backends..model"), None);
        assert_eq!(ConfigSetKey::parse("backends.claude"), None);
    }

    #[test]
    fn backend_fields_default_to_project_scope() {
        let key = ConfigSetKey::parse("backends.claude.model").expect("parse backend model");

        assert_eq!(Scope::default_for_key(key.scope_key()), Scope::Project);
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("bad config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("bad config: {0}")]
    Encode(#[from] toml::ser::Error),

    // paths
    #[error("cannot locate user config directory")]
    NoConfigDir,
    #[error("cannot locate user cache directory")]
    NoCacheDir,
    #[error("cannot locate user data directory")]
    NoDataDir,

    // tree
    #[error("no config found, ace setup?")]
    NoConfig,

    // school specifier (parsed by school/linked.rs)
    #[error("traversal in source: {0}")]
    TraversalInSource(String),
    #[error("traversal in path: {0}")]
    TraversalInPath(String),
}
