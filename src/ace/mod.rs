pub mod io;

use std::path::{Path, PathBuf};

use once_cell::unsync::OnceCell;

use crate::backend::registry::TemplateCtx;
use crate::backend::{Backend, BackendError, Kind, registry};
use crate::config::ace_toml::AceToml;
use crate::config::paths::AcePaths;
use crate::config::resolve;
use crate::config::resolve::Resolved;
use crate::config::tree::Tree;
use crate::config::{ConfigError, Scope};
use crate::git::Git;
use crate::school::linked::LinkedSchool;
use crate::school::toml::SchoolToml;
use crate::school::{School, SchoolError, toml as school_toml};
use crate::skills::{Decided, SkillError, Skills};

pub use io::{Io, IoError, partition_picked};

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
    paths: AcePaths,
    tree: OnceCell<Tree>,
    resolved: OnceCell<Resolved>,
    backend: OnceCell<Backend>,
    linked_school: OnceCell<LinkedSchool>,
    school_toml: OnceCell<Option<SchoolToml>>,
    school: OnceCell<Option<School>>,
    skills: OnceCell<Skills<Decided>>,
    overrides: AceToml,
    scope_override: Option<Scope>,
    io: Io,
}

impl Ace {
    /// `paths` is resolved by the caller — the ambient user config/cache
    /// directories enter the process exactly once, at the edge, so nothing
    /// downstream reads the host environment.
    pub fn new(project_dir: PathBuf, paths: AcePaths, io: Io) -> Self {
        Self {
            project_dir,
            paths,
            tree: OnceCell::new(),
            resolved: OnceCell::new(),
            backend: OnceCell::new(),
            linked_school: OnceCell::new(),
            school_toml: OnceCell::new(),
            school: OnceCell::new(),
            skills: OnceCell::new(),
            overrides: AceToml::default(),
            scope_override: None,
            io,
        }
    }

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    pub fn should_colorize(&self) -> bool {
        self.io.should_colorize()
    }

    pub fn can_ask(&self) -> bool {
        self.io.can_ask()
    }

    pub fn silence(&mut self) {
        self.io.silence();
    }

    /// Replace the runtime-override layer wholesale. The CLI builds an
    /// `AceToml` from global flags (--backend, --trust, --session-prompt,
    /// --env, ...) and hands it in once at startup. Higher-priority than
    /// any on-disk layer (see `docs/spec/architecture.md`).
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
        self.tree.get_or_try_init(|| Tree::load(&self.paths))
    }

    pub fn set_scope_override(&mut self, scope: Option<Scope>) {
        self.scope_override = scope;
    }

    pub fn scope_override(&self) -> Option<Scope> {
        self.scope_override
    }

    pub fn paths(&self) -> &AcePaths {
        &self.paths
    }

    /// Lazy-load tree + school.toml + run the merge into the effective config.
    /// Idempotent. The backend binding is *not* eagerly resolved here —
    /// `backend()` does that on demand so read-only commands survive a stale
    /// selector.
    pub fn require_config(&self) -> Result<&Resolved, ConfigError> {
        self.resolved.get_or_try_init(|| {
            let tree = self.require_tree()?;
            let school = self.school_toml()?;
            Ok(resolve::merge(tree, school, &self.overrides))
        })
    }

    /// Lazy-load the linked school's `school.toml` content, raw. `Ok(None)`
    /// when no school is configured, the school is uninitialized, or the file
    /// is missing; a present-but-malformed file errors loudly. Prefer
    /// `school()` for the bound domain view; this is for merge input and
    /// config introspection, which need fields (backend, backends) the domain
    /// view deliberately drops.
    pub fn school_toml(&self) -> Result<Option<&SchoolToml>, ConfigError> {
        let cached = self.school_toml.get_or_try_init(|| {
            let linked = match self.require_linked_school() {
                Ok(linked) => linked,
                Err(SchoolError::TreeLoad(e)) => return Err(e),
                Err(SchoolError::NoSpecifier | SchoolError::NotInitialized) => return Ok(None),
                Err(SchoolError::NoSchool) => {
                    unreachable!("only require_authoring_school mints NoSchool")
                }
            };

            school_toml::load_optional(&linked.toml_path())
        })?;
        Ok(cached.as_ref())
    }

    /// Lazy-load the resolved Backend binding (registry build + name lookup).
    /// `Err(BackendError::Unknown(_))` when the selector points at a name
    /// that isn't a built-in or declared `[[backends]]`.
    pub fn backend(&self) -> Result<&Backend, BackendError> {
        self.backend.get_or_try_init(|| {
            let resolved = self.require_config()?;
            registry::bind(resolved, &self.template_ctx())
        })
    }

    /// Names that resolve as a backend selector: built-ins plus every
    /// `[[backends]]` declaration across school/user/project/local layers.
    /// Sorted, deduped. Used for selector validation and recovery prompts.
    pub fn known_backend_names(&self) -> Result<Vec<String>, ConfigError> {
        let resolved = self.require_config()?;

        let mut names: Vec<String> = Kind::ALL.iter().map(|k| k.name().to_string()).collect();
        names.extend(resolved.backend_decls.iter().map(|d| d.value.name.clone()));

        names.sort();
        names.dedup();
        Ok(names)
    }

    /// Build the `TemplateCtx` used to render `{{ ... }}` placeholders inside
    /// `[[backends]].cmd` and `env`. Unresolved school path → empty string,
    /// matching the unknown-placeholder rule in
    /// `docs/spec/backend.md § Path Templating`.
    fn template_ctx(&self) -> TemplateCtx {
        let school_dir = self
            .require_linked_school()
            .ok()
            .map(|p| p.root.display().to_string())
            .unwrap_or_default();
        let project_dir = self.project_dir.display().to_string();
        let home = std::env::var("HOME").unwrap_or_default();
        TemplateCtx {
            school_dir,
            project_dir,
            home,
        }
    }

    /// Union of `exclude_mcp` across user/project/local scopes. Empty when no
    /// tree is loaded; callers needing a guarantee should `require_config`
    /// or `require_tree` first.
    pub fn excluded_mcp(&self) -> std::collections::HashSet<String> {
        let Ok(tree) = self.require_tree() else {
            return std::collections::HashSet::new();
        };
        let mut out = std::collections::HashSet::new();
        for toml in [&tree.user, &tree.project, &tree.local]
            .iter()
            .copied()
            .flatten()
        {
            for name in &toml.exclude_mcp {
                out.insert(name.clone());
            }
        }
        out
    }

    /// Names of school skills filtered out by the resolved
    /// `include_skills` / `exclude_skills` rules. Sorted, deduped. Empty when
    /// skills can't be resolved (no school, discovery I/O failure) — surfacing
    /// the empty set is preferable to bubbling errors into the session prompt.
    pub fn excluded_skills(&self) -> Vec<String> {
        let Ok(skills) = self.skills() else {
            return Vec::new();
        };
        let mut names: Vec<String> = skills
            .excluded()
            .map(|s| s.locator.as_str().to_string())
            .collect();
        names.sort();
        names.dedup();
        names
    }

    /// Resolve school paths. See docs/spec/school/overview.md (Context Resolution)
    /// for the full case matrix. Summary:
    ///
    /// - Always resolves via `ace.toml`'s specifier. The presence of `school.toml`
    ///   in the workdir is *content*, not a location signal — a school repo that
    ///   wants to dogfood itself uses `school = "."` in its own `ace.toml`.
    /// - `require_tree` → specifier → `LinkedSchool::resolve`.
    /// - Resolved root exists as a dir but lacks `school.toml` → `NotInitialized`
    ///   (covers cases 5 and 7: local pre-init / clone uninitialized upstream).
    /// - Resolved root absent → Ok; cmd/pull.rs self-heals via clone (case 8).
    pub fn require_linked_school(&self) -> Result<&LinkedSchool, SchoolError> {
        self.linked_school.get_or_try_init(|| {
            let tree = self.require_tree()?;
            let Some(spec) = tree.specifier() else {
                return Err(SchoolError::NoSpecifier);
            };
            let linked = LinkedSchool::resolve(&self.project_dir, &spec)?;
            if linked.root.is_dir() && !linked.toml_path().exists() {
                return Err(SchoolError::NotInitialized);
            }
            Ok(linked)
        })
    }

    /// Root of the authored school: `Some(project_dir)` iff the workdir holds
    /// a `school.toml`. See `school::authored_root`.
    pub fn authored_school_root(&self) -> Option<PathBuf> {
        crate::school::authored_root(&self.project_dir)
    }

    /// Resolve the school an authoring command (`ace school pull` / `skills` /
    /// `validate`, `ace import`) operates on. Cwd-first per
    /// docs/spec/school/school-commands.md:
    ///
    /// 1. `cwd/school.toml` exists → the authored school is the cwd.
    /// 2. Otherwise → the linked school, announced with a warning — the
    ///    fallback is never silent.
    /// 3. Neither → `NoSchool`, whose hint names both bootstrap routes.
    pub fn require_authoring_school(&mut self) -> Result<PathBuf, SchoolError> {
        if let Some(root) = self.authored_school_root() {
            return Ok(root);
        }

        let linked = match self.require_linked_school() {
            Ok(paths) => paths.root.clone(),
            Err(SchoolError::NoSpecifier | SchoolError::TreeLoad(ConfigError::NoConfig)) => {
                return Err(SchoolError::NoSchool);
            }
            Err(e) => return Err(e),
        };

        self.warn(&format!(
            "no school.toml in current directory — using the linked school at {}",
            linked.display()
        ));
        Ok(linked)
    }

    /// Drop every school-derived cache so the next accessors re-read from
    /// disk. Used after clone-on-first-run, when school.toml newly exists.
    pub fn invalidate_school_caches(&mut self) {
        self.linked_school = OnceCell::new();
        self.school_toml = OnceCell::new();
        self.school = OnceCell::new();
        self.skills = OnceCell::new();
        self.invalidate_resolved();
    }

    /// Lazy-load the resolved School binding. `Ok(None)` when no school is
    /// configured or school.toml is missing/unreadable. Does NOT require the
    /// backend to resolve, so read-only inspection paths still work when the
    /// selector points at an unknown backend.
    pub fn school(&self) -> Result<Option<&School>, SchoolError> {
        let cached = self.school.get_or_try_init(|| -> Result<_, SchoolError> {
            Ok(self.school_toml()?.map(|st| School::from(st.clone())))
        })?;
        Ok(cached.as_ref())
    }

    /// Lazy-load the resolved SkillSet — discover the school's `skills/` tree
    /// and resolve against the layered config. Errors when no school is
    /// configured (skills require a school root) or discovery I/O fails.
    pub fn skills(&self) -> Result<&Skills<Decided>, SkillError> {
        self.skills.get_or_try_init(|| {
            let school_root = &self.require_linked_school()?.root;
            let tree = self.require_tree()?;
            // Discovery prunes (malformed identities) are surfaced at the
            // write/import boundaries (link/setup, pull, import). This getter is
            // `&self`-cached and takes no action on a pruned skill, so it neither
            // re-warns nor needs to.
            let (discovered, _prunes) = Skills::discover(school_root)?;
            let (validated, rejected) = discovered.validate();
            Ok(validated.resolve(tree).with_rejected(rejected))
        })
    }

    pub fn git<'a>(&self, repo: &'a Path) -> Git<'a> {
        Git::new(repo, self.io.should_colorize())
    }

    // -- output --

    #[allow(dead_code)]
    pub fn enter_alt_screen(&self) {
        self.io.enter_alt_screen();
    }

    pub fn progress(&mut self, msg: &str) {
        self.io.progress(msg);
    }

    pub fn done(&mut self, msg: &str) {
        self.io.done(msg);
    }

    pub fn info(&mut self, msg: &str) {
        self.io.info(msg);
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

    pub fn prompt_select(&mut self, prompt: &str, options: Vec<String>) -> Result<String, IoError> {
        self.io.prompt_select(prompt, options)
    }

    pub fn prompt_multiselect(
        &mut self,
        prompt: &str,
        options: Vec<String>,
        default_all: bool,
    ) -> Result<Vec<usize>, IoError> {
        self.io.prompt_multiselect(prompt, options, default_all)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// These tests exercise resolution, not output, so the session is built
    /// with presentation off.
    /// Every layer roots inside `dir`, so no test reads the host's real user
    /// config — a stray `~/.config/ace/ace.toml` must not change an outcome.
    fn ace_at(dir: &std::path::Path) -> Ace {
        let paths = AcePaths {
            user: dir.join("config/ace.toml"),
            project: dir.join("ace.toml"),
            local: dir.join("ace.local.toml"),
            cache: dir.join("cache"),
        };
        Ace::new(dir.to_path_buf(), paths, Io::new(true, false))
    }

    /// docs/spec/school/overview.md case 5: ace.toml with local specifier `.`,
    /// no school.toml at resolved root → MissingSchool, not Ok with stale paths.
    #[test]
    fn require_linked_school_local_specifier_uninitialized_returns_not_initialized() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("ace.toml"),
            "school = \".\"\nbackend = \"flaude\"\n",
        )
        .unwrap();
        let ace = ace_at(tmp.path());
        let err = ace
            .require_linked_school()
            .expect_err("expected NotInitialized");
        assert!(matches!(err, SchoolError::NotInitialized), "got: {err:?}");
        assert_eq!(
            err.hint(),
            Some("run `ace school init` to bootstrap this repo as a school")
        );
    }

    /// School-repo dogfood: ace.toml with `school = "."` plus a workdir school.toml
    /// resolves to the workdir via the specifier — *not* via marker-file detection.
    /// Removing the ace.toml would (correctly) yield NoSpecifier even though
    /// school.toml is present.
    #[test]
    fn require_linked_school_embedded_specifier_resolves_to_workdir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("ace.toml"),
            "school = \".\"\nbackend = \"flaude\"\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("school.toml"), "name = \"x\"\n").unwrap();
        let ace = ace_at(tmp.path());
        let paths = ace.require_linked_school().expect("Ok");
        assert_eq!(paths.root, tmp.path());
    }

    /// Without an ace.toml specifier, a bare school.toml in the workdir no longer
    /// short-circuits — resolution always goes through the tree → specifier path.
    /// With no ace.toml present at all, tree load fails first (case 2 in the
    /// matrix: intent unknowable).
    #[test]
    fn require_linked_school_workdir_school_toml_without_ace_toml_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("school.toml"), "name = \"x\"\n").unwrap();
        let ace = ace_at(tmp.path());
        let err = ace.require_linked_school().expect_err("expected error");
        assert!(matches!(err, SchoolError::TreeLoad(_)), "got: {err:?}");
    }

    /// A present-but-malformed school.toml errors loudly instead of silently
    /// resolving as "no school" — otherwise a typo makes the school's
    /// backend/backends/mcp vanish from the merge with zero diagnostic.
    #[test]
    fn school_toml_malformed_file_errors() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("ace.toml"),
            "school = \".\"\nbackend = \"flaude\"\n",
        )
        .unwrap();
        std::fs::write(tmp.path().join("school.toml"), "name = [not toml\n").unwrap();
        let ace = ace_at(tmp.path());
        ace.school_toml().expect_err("expected parse error");
    }

    /// docs/spec/school/overview.md case 3: ace.toml without specifier → Missing.
    #[test]
    fn require_linked_school_no_specifier_returns_no_specifier() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("ace.toml"), "backend = \"flaude\"\n").unwrap();
        let ace = ace_at(tmp.path());
        let err = ace
            .require_linked_school()
            .expect_err("expected NoSpecifier");
        assert!(matches!(err, SchoolError::NoSpecifier), "got: {err:?}");
        assert_eq!(err.hint(), Some("run `ace setup` to choose a school"));
    }
}
