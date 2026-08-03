use super::ConfigError;
use super::ace_toml::{self, AceToml};
use super::paths::AcePaths;

/// Raw `ace.toml` layers parsed from disk. `None` means "no file present" —
/// distinct from "present but empty" so diagnostics can tell the two apart.
/// School content is not a layer here: `crate::school` owns its location and
/// loading, and the merge receives it as a separate input.
#[derive(Clone, Default)]
pub struct Tree {
    pub user: Option<AceToml>,
    pub project: Option<AceToml>,
    pub local: Option<AceToml>,
}

impl Tree {
    pub fn load(paths: &AcePaths) -> Result<Self, ConfigError> {
        // A layer file is optional; a present one must parse. Probe first so
        // the loader keeps a single strict contract.
        let user = match paths.user.exists() {
            true => Some(ace_toml::load(&paths.user)?),
            false => None,
        };
        let project = match paths.project.exists() {
            true => Some(ace_toml::load(&paths.project)?),
            false => None,
        };
        let local = match paths.local.exists() {
            true => Some(ace_toml::load(&paths.local)?),
            false => None,
        };

        // Any layer is a signal of intent; a user-level school is the default for
        // every project that doesn't override it. Nothing anywhere is unknowable.
        if user.is_none() && project.is_none() && local.is_none() {
            return Err(ConfigError::NoConfig);
        }

        Ok(Tree {
            user,
            project,
            local,
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
