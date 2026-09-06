pub mod ace_toml;
pub mod index_toml;
pub mod inspection;
pub mod paths;
pub mod resolve;
pub mod selection;
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

/// One key surface shared by inspection and mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigKey {
    School,
    Backend,
    Trust,
    Resume,
    SkipUpdate,
    SessionPrompt,
    Env(String),
    BackendField {
        name: String,
        field: BackendConfigField,
    },
    Selection(SelectionField),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendConfigField {
    Model,
    Effort,
}

impl BackendConfigField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Model => "model",
            Self::Effort => "effort",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionField {
    Skills,
    IncludeSkills,
    ExcludeSkills,
    ExcludeMcp,
}

impl SelectionField {
    pub fn label(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::IncludeSkills => "include_skills",
            Self::ExcludeSkills => "exclude_skills",
            Self::ExcludeMcp => "exclude_mcp",
        }
    }
}

impl ConfigKey {
    pub const SCALARS: [Self; 6] = [
        Self::School,
        Self::Backend,
        Self::Trust,
        Self::Resume,
        Self::SkipUpdate,
        Self::SessionPrompt,
    ];
    pub const SELECTIONS: [Self; 4] = [
        Self::Selection(SelectionField::Skills),
        Self::Selection(SelectionField::IncludeSkills),
        Self::Selection(SelectionField::ExcludeSkills),
        Self::Selection(SelectionField::ExcludeMcp),
    ];

    pub fn parse(key: &str) -> Option<Self> {
        if let Some(name) = key.strip_prefix("env.") {
            return (!name.is_empty()).then(|| Self::Env(name.to_string()));
        }
        if let Some(path) = key.strip_prefix("backends.") {
            let (name, field) = path.rsplit_once('.')?;
            if name.is_empty() {
                return None;
            }
            let field = match field {
                "model" => BackendConfigField::Model,
                "effort" => BackendConfigField::Effort,
                _ => return None,
            };
            return Some(Self::BackendField {
                name: name.to_string(),
                field,
            });
        }
        Self::SCALARS
            .into_iter()
            .chain(Self::SELECTIONS)
            .find(|candidate| candidate.scope_key() == key)
    }

    pub fn scope_key(&self) -> &str {
        match self {
            Self::School => "school",
            Self::Backend => "backend",
            Self::Trust => "trust",
            Self::Resume => "resume",
            Self::SkipUpdate => "skip_update",
            Self::SessionPrompt => "session_prompt",
            Self::Env(_) => "env",
            Self::BackendField { .. } => "backends",
            Self::Selection(field) => field.label(),
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Env(name) => format!("env.{name}"),
            Self::BackendField { name, field } => format!("backends.{name}.{}", field.label()),
            _ => self.scope_key().to_string(),
        }
    }

    pub fn is_writable(&self) -> bool {
        !matches!(self, Self::Selection(_))
    }

    pub fn is_personal(&self) -> bool {
        matches!(self, Self::Trust | Self::Resume)
    }

    pub fn is_union(&self) -> bool {
        matches!(
            self,
            Self::Selection(
                SelectionField::IncludeSkills
                    | SelectionField::ExcludeSkills
                    | SelectionField::ExcludeMcp
            )
        )
    }

    pub fn display_name(&self) -> String {
        use ace_toml::display_key_segment;
        match self {
            Self::Env(name) => format!("env.{}", display_key_segment(name)),
            Self::BackendField { name, field } => {
                format!("backends.{}.{}", display_key_segment(name), field.label())
            }
            _ => self.name(),
        }
    }

    pub fn help() -> String {
        let simple = Self::SCALARS
            .into_iter()
            .map(|key| key.name())
            .collect::<Vec<_>>()
            .join(", ");
        let selections = Self::SELECTIONS
            .into_iter()
            .map(|key| key.name())
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{simple}, env.KEY, backends.NAME.model, backends.NAME.effort; read-only: {selections}"
        )
    }
}

#[cfg(test)]
mod config_key_tests {
    use super::*;

    #[test]
    fn backend_field_uses_the_terminal_field_and_literal_instance_name() {
        assert_eq!(
            ConfigKey::parse("backends.team.codex.effort"),
            Some(ConfigKey::BackendField {
                name: "team.codex".into(),
                field: BackendConfigField::Effort,
            })
        );
        assert_eq!(ConfigKey::parse("backends..model"), None);
        assert_eq!(ConfigKey::parse("backends.codex.cmd"), None);
        assert!(
            !ConfigKey::parse("skills")
                .expect("readable key")
                .is_writable()
        );
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("bad config: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("bad config document: {0}")]
    Document(#[from] toml_edit::TomlError),
    #[error("bad config edit: {0}")]
    InvalidEdit(String),
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
