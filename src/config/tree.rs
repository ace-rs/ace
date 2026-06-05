use std::path::Path;

use super::ConfigError;
use super::ace_toml::{self, AceToml};
use super::paths::AcePaths;
use super::school_paths;
use super::school_toml::{self, SchoolToml};

/// Raw config layers parsed from disk. `None` means "no file present" — distinct
/// from "present but empty" so diagnostics can tell the two apart. Derived
/// fields (school paths, the school's contributed backend name) are computed
/// downstream by the resolver and binding layers.
#[derive(Clone, Default)]
pub struct Tree {
    pub user: Option<AceToml>,
    pub project: Option<AceToml>,
    pub local: Option<AceToml>,
    pub school: Option<SchoolToml>,
}

impl Tree {
    pub fn load(paths: &AcePaths) -> Result<Self, ConfigError> {
        let user = load_optional(&paths.user)?;
        let project = load_optional(&paths.project)?;
        let local = load_optional(&paths.local)?;

        // User layer alone doesn't mean a project is set up.
        if project.is_none() && local.is_none() {
            return Err(ConfigError::NoConfig);
        }

        Ok(Tree {
            user,
            project,
            local,
            school: None,
        })
    }

    /// Resolve school specifier from ace.toml layers (last non-empty wins).
    pub fn specifier(&self) -> Option<String> {
        [&self.local, &self.project, &self.user]
            .iter()
            .filter_map(|opt| opt.as_ref())
            .find(|l| !l.school.is_empty())
            .map(|l| l.school.clone())
    }

    /// Second pass: read school.toml from the resolved specifier's clone path.
    /// No-op when no specifier is set or school.toml is missing/unreadable.
    pub fn load_school(&mut self, project_dir: &Path) -> Result<(), ConfigError> {
        let Some(spec) = self.specifier() else {
            return Ok(());
        };

        let sp = school_paths::resolve(project_dir, &spec)?;
        let school_toml_path = sp.root.join("school.toml");
        if school_toml_path.exists()
            && let Ok(st) = school_toml::load(&school_toml_path)
        {
            self.school = Some(st);
        }
        Ok(())
    }

    /// Backend name contributed by the school layer, if any.
    pub fn school_backend(&self) -> Option<&str> {
        self.school.as_ref().and_then(|s| s.backend.as_deref())
    }
}

fn load_optional(path: &Path) -> Result<Option<AceToml>, ConfigError> {
    match ace_toml::load(path) {
        Ok(config) => Ok(Some(config)),
        Err(ConfigError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}
