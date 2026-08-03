pub mod linked;
pub mod toml;

use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use self::toml::{ImportDecl, McpDecl, Project, SchoolToml};
use crate::config::ConfigError;

/// Errors that can occur while binding a school. Wraps `ConfigError` for
/// the underlying tree/load step and adds school-specific failure modes.
#[derive(Debug, thiserror::Error)]
pub enum SchoolError {
    #[error(transparent)]
    TreeLoad(#[from] ConfigError),
    #[error("no school configured")]
    NoSpecifier,
    #[error("school not initialized")]
    NotInitialized,
    #[error("no school here and none linked")]
    NoSchool,
}

impl SchoolError {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::NoSpecifier => Some("run `ace setup` to choose a school"),
            Self::NotInitialized => {
                Some("run `ace school init` to bootstrap this repo as a school")
            }
            Self::NoSchool => {
                Some("run `ace school init` to author a school here, or `ace setup` to link one")
            }
            Self::TreeLoad(_) => None,
        }
    }
}

/// Root of the authored school: `Some(dir)` iff the directory holds a
/// `school.toml`. Location probe only — the file's content is never read.
/// See the glossary in docs/spec/school/overview.md.
pub fn authored_root(dir: &Path) -> Option<PathBuf> {
    dir.join("school.toml").exists().then(|| dir.to_path_buf())
}

#[derive(Debug, Default, Serialize)]
pub struct School {
    pub name: String,
    pub session_prompt: String,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub env: HashMap<String, String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mcp: Vec<McpDecl>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<Project>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub imports: Vec<ImportDecl>,
}

impl From<SchoolToml> for School {
    fn from(st: SchoolToml) -> Self {
        Self {
            name: st.name,
            session_prompt: st.session_prompt,
            env: st.env,
            mcp: st.mcp,
            projects: st.projects,
            imports: st.imports,
        }
    }
}
