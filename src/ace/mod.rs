pub mod io;

use std::path::{Path, PathBuf};

use once_cell::unsync::OnceCell;

use crate::backend::{registry, Backend, BackendError};
use crate::config;
use crate::config::ace_toml::AceToml;
use crate::config::paths::AcePaths;
use crate::config::school_paths::SchoolPaths;
use crate::config::tree::Tree;
use crate::config::{ConfigError, Scope};
use crate::git::Git;
use crate::resolver;
use crate::resolver::Resolved;
use crate::school::{School, SchoolError};
use crate::skills::{Decided, SkillError, Skills};

pub use io::{logo, IoError, OutputMode};
use io::Io;

/// Lazy-cached session view. All read accessors take `&self` and populate
/// their cell on first call via `OnceCell`. Mutations (overrides, reload)
/// take `&mut self` and reset cells by reassignment — there is no
/// in-place invalidation API on `OnceCell`.
///
/// Failed loads are not memoized: `OnceCell::get_or_try_init` returns the
/// error and leaves the cell empty, so the next call retries. This matches
/// how `Option<T>` caching behaved before the migration.
pub struct Ace {
    project_dir: PathBuf,
    tree: OnceCell<Tree>,
    resolved: OnceCell<Resolved>,
    backend: OnceCell<Backend>,
    school_paths: OnceCell<SchoolPaths>,
    school: OnceCell<Option<School>>,
    skills: OnceCell<Skills<Decided>>,
    overrides: AceToml,
    scope_override: Option<Scope>,
    io: Io,
    mode: OutputMode,
}

impl Ace {
    pub fn new(project_dir: PathBuf, mode: OutputMode) -> Self {
        Self {
            project_dir,
            tree: OnceCell::new(),
            resolved: OnceCell::new(),
            backend: OnceCell::new(),
            school_paths: OnceCell::new(),
            school: OnceCell::new(),
            skills: OnceCell::new(),
            overrides: AceToml::default(),
            scope_override: None,
            io: Io::new(mode),
            mode,
        }
    }

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    pub fn mode(&self) -> OutputMode {
        self.mode
    }

    /// Replace the runtime-override layer wholesale. The CLI builds an
    /// `AceToml` from global flags (--backend, --trust, --session-prompt,
    /// --env, ...) and hands it in once at startup. Higher-priority than
    /// any on-disk layer (see `spec/decisions/007.md`).
    pub fn set_overrides(&mut self, overrides: AceToml) {
        self.overrides = overrides;
        self.invalidate_resolved();
    }

    /// Set just the backend field on the override layer. Used by the
    /// PROD9-146 recovery picker when an unknown backend selector is
    /// re-pointed mid-session.
    pub fn override_backend(&mut self, backend: String) {
        self.overrides.backend = Some(backend);
        self.invalidate_resolved();
    }

    pub fn overrides(&self) -> &AceToml {
        &self.overrides
    }

    fn invalidate_resolved(&mut self) {
        self.resolved = OnceCell::new();
        self.backend = OnceCell::new();
    }

    /// Lazy-load the raw config tree (parse-only; no merge, no binding).
    /// Survives `State::resolve` failures, so recovery code paths can still
    /// inspect declared `[[backends]]` after an unknown-backend error.
    pub fn require_tree(&self) -> Result<&Tree, ConfigError> {
        self.tree.get_or_try_init(|| {
            let paths = config::paths::resolve(&self.project_dir)?;
            let mut tree = Tree::load(&paths)?;
            tree.load_school(&self.project_dir)?;
            Ok(tree)
        })
    }

    pub fn set_scope_override(&mut self, scope: Option<Scope>) {
        self.scope_override = scope;
    }

    pub fn scope_override(&self) -> Option<Scope> {
        self.scope_override
    }

    /// Resolve config paths for the current project directory.
    pub fn require_paths(&self) -> Result<AcePaths, ConfigError> {
        config::paths::resolve(&self.project_dir)
    }

    /// Lazy-load tree + school.toml + run the merge. Idempotent. The backend
    /// binding is *not* eagerly resolved here — `backend()` does that on
    /// demand so read-only commands survive a stale selector.
    pub fn require_resolved(&self) -> Result<&Resolved, ConfigError> {
        self.resolved.get_or_try_init(|| {
            let tree = self.require_tree()?;
            Ok(resolver::merge(tree, &self.overrides))
        })
    }

    /// Lazy-load the resolved Backend binding (registry build + name lookup).
    /// `Err(BackendError::Unknown(_))` when the selector points at a name
    /// that isn't a built-in or declared `[[backends]]`.
    pub fn backend(&self) -> Result<&Backend, BackendError> {
        self.backend.get_or_try_init(|| {
            let resolved = self.require_resolved()?;
            registry::bind(resolved)
        })
    }

    /// Resolve school paths. See spec/school/overview.md (Context Resolution)
    /// for the full case matrix. Summary:
    /// - school.toml in workdir → Ok (school-repo).
    /// - Else require_tree → specifier → resolve.
    /// - Resolved root exists but lacks school.toml → MissingSchool
    ///   (covers cases 5 and 7: local pre-init / clone uninitialized upstream).
    /// - Resolved root absent → Ok; cmd/pull.rs self-heals via clone (case 8).
    pub fn require_school(&self) -> Result<&SchoolPaths, SchoolError> {
        self.school_paths.get_or_try_init(|| {
            if self.project_dir.join("school.toml").exists() {
                return Ok(SchoolPaths {
                    source: ".".to_string(),
                    clone_path: None,
                    root: self.project_dir.clone(),
                });
            }
            let tree = self.require_tree()?;
            let Some(spec) = tree.specifier() else {
                return Err(SchoolError::NoSpecifier);
            };
            let paths = config::school_paths::resolve(&self.project_dir, &spec)?;
            if paths.root.is_dir() && !paths.root.join("school.toml").exists() {
                return Err(SchoolError::NotInitialized);
            }
            Ok(paths)
        })
    }

    /// Re-read school.toml from disk and invalidate downstream caches so the
    /// next accessors derive from the freshly loaded tree. Used after
    /// clone-on-first-run.
    pub fn reload_tree(&mut self) -> Result<&Resolved, ConfigError> {
        let mut tree = self.tree.take().ok_or(ConfigError::NoConfig)?;
        tree.load_school(&self.project_dir)?;
        self.tree = OnceCell::from(tree);
        self.invalidate_resolved();
        self.school_paths = OnceCell::new();
        self.school = OnceCell::new();
        self.skills = OnceCell::new();
        self.require_resolved()
    }

    /// Lazy-load the resolved School binding. `Ok(None)` when no school is
    /// configured or school.toml is missing/unreadable. Does NOT require the
    /// backend to resolve, so read-only inspection paths still work when the
    /// selector points at an unknown backend.
    pub fn school(&self) -> Result<Option<&School>, SchoolError> {
        let cached = self.school.get_or_try_init(|| -> Result<_, SchoolError> {
            let tree = self.require_tree()?;
            Ok(tree.school.as_ref().map(|st| School::from(st.clone())))
        })?;
        Ok(cached.as_ref())
    }

    /// Lazy-load the resolved SkillSet — discover the school's `skills/` tree
    /// and resolve against the layered config. Errors when no school is
    /// configured (skills require a school root) or discovery I/O fails.
    pub fn skills(&self) -> Result<&Skills<Decided>, SkillError> {
        self.skills.get_or_try_init(|| {
            let school_root = &self.require_school()?.root;
            let discovered = Skills::discover(school_root)?;
            let tree = self.require_tree()?;
            Ok(discovered.resolve(tree))
        })
    }

    pub fn git<'a>(&self, repo: &'a Path) -> Git<'a> {
        Git::new(repo, self.mode)
    }

    // -- output --

    pub fn enter_alt_screen(&self) {
        self.io.enter_alt_screen();
    }

    pub fn progress(&mut self, msg: &str) {
        self.io.progress(msg);
    }

    pub fn done(&mut self, msg: &str) {
        self.io.done(msg);
    }

    pub fn warn(&mut self, msg: &str) {
        self.io.warn(msg);
    }

    pub fn error(&mut self, msg: &str) {
        self.io.error(msg);
    }

    pub fn hint(&mut self, msg: &str) {
        self.io.hint(msg);
    }

    pub fn data(&mut self, msg: &str) {
        self.io.data(msg);
    }

    pub fn separator(&mut self) {
        self.io.separator();
    }

    // -- input --

    pub fn prompt_text(&mut self, prompt: &str, initial: Option<&str>) -> Result<String, IoError> {
        self.io.prompt_text(prompt, initial)
    }

    pub fn prompt_confirm(&mut self, prompt: &str, default: bool) -> Result<bool, IoError> {
        self.io.prompt_confirm(prompt, default)
    }

    pub fn prompt_select(&mut self, prompt: &str, options: Vec<String>) -> Result<String, IoError> {
        self.io.prompt_select(prompt, options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// spec/school/overview.md case 5: ace.toml with local specifier `.`,
    /// no school.toml at resolved root → MissingSchool, not Ok with stale paths.
    #[test]
    fn require_school_local_specifier_uninitialized_returns_not_initialized() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("ace.toml"),
            "school = \".\"\nbackend = \"flaude\"\n",
        )
        .unwrap();
        let ace = Ace::new(tmp.path().to_path_buf(), OutputMode::Silent);
        let err = ace.require_school().expect_err("expected NotInitialized");
        assert!(
            matches!(err, SchoolError::NotInitialized),
            "got: {err:?}"
        );
        assert!(err.to_string().contains("ace school init"), "msg: {err}");
    }

    /// spec/school/overview.md case 1: school.toml in workdir short-circuits to Ok.
    #[test]
    fn require_school_workdir_school_toml_returns_ok() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("school.toml"), "name = \"x\"\n").unwrap();
        let ace = Ace::new(tmp.path().to_path_buf(), OutputMode::Silent);
        let paths = ace.require_school().expect("Ok");
        assert_eq!(paths.root, tmp.path());
        assert!(paths.clone_path.is_none());
    }

    /// spec/school/overview.md case 3: ace.toml without specifier → Missing.
    #[test]
    fn require_school_no_specifier_returns_no_specifier() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("ace.toml"), "backend = \"flaude\"\n").unwrap();
        let ace = Ace::new(tmp.path().to_path_buf(), OutputMode::Silent);
        let err = ace.require_school().expect_err("expected NoSpecifier");
        assert!(matches!(err, SchoolError::NoSpecifier), "got: {err:?}");
        assert!(err.to_string().contains("ace setup"), "msg: {err}");
    }
}
