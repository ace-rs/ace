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

        // Any layer is a signal of intent; a user-level school is the default for
        // every project that doesn't override it. Nothing anywhere is unknowable.
        if user.is_none() && project.is_none() && local.is_none() {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `AcePaths` rooted entirely inside a tempdir, so no test reads the host's
    /// real user config.
    fn paths_in(dir: &Path) -> AcePaths {
        AcePaths {
            user: dir.join("user/ace.toml"),
            project: dir.join("project/ace.toml"),
            local: dir.join("project/ace.local.toml"),
            cache: dir.join("cache"),
        }
    }

    fn write_school(path: &Path, specifier: &str) {
        std::fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        std::fs::write(path, format!("school = \"{specifier}\"\n")).expect("write");
    }

    #[test]
    fn load_accepts_user_layer_alone() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = paths_in(tmp.path());
        write_school(&paths.user, "ace-rs/school");

        let tree = Tree::load(&paths).expect("user layer alone is a valid setup");

        assert_eq!(tree.specifier().as_deref(), Some("ace-rs/school"));
    }

    #[test]
    fn load_errors_when_every_layer_absent() {
        let tmp = tempfile::tempdir().expect("tempdir");

        let err = Tree::load(&paths_in(tmp.path()))
            .err()
            .expect("no config anywhere");

        assert!(matches!(err, ConfigError::NoConfig), "got {err:?}");
    }

    #[test]
    fn specifier_prefers_local_then_project_then_user() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let paths = paths_in(tmp.path());
        write_school(&paths.user, "user/school");
        write_school(&paths.project, "project/school");

        let tree = Tree::load(&paths).expect("load");
        assert_eq!(tree.specifier().as_deref(), Some("project/school"));

        write_school(&paths.local, "local/school");
        let tree = Tree::load(&paths).expect("load");
        assert_eq!(tree.specifier().as_deref(), Some("local/school"));
    }
}
