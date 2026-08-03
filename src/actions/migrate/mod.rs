mod host_scoped_imports;
mod legacy_cache_layout;

use std::path::PathBuf;

use crate::ace::Ace;
use crate::config::ConfigError;
use crate::config::index_toml::{self, IndexToml, LAYOUT_VERSION};

/// On-disk layout migrations. See `docs/spec/migrations.md`.
///
/// The version ACE last migrated to is recorded as `layout_version` in `index.toml`,
/// dated rather than counted, so a step lines up with the decision doc behind it. Steps
/// run oldest-first at startup, and re-derivable state is torn down and re-fetched rather
/// than transformed in place.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error("{path} is from a newer ace (layout {found})")]
    FromTheFuture { path: PathBuf, found: String },
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl MigrateError {
    /// Recovery hint, per `docs/spec/ux.md` §3. Wrapped variants leave it to
    /// their own leaf error.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::FromTheFuture { .. } => Some("upgrade ace to use this install"),
            Self::Config(_) | Self::Io(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    LegacyCacheLayout,
    HostScopedImports,
}

impl Step {
    /// Oldest first. The last entry is what `LAYOUT_VERSION` must name.
    const ALL: [Step; 2] = [Step::LegacyCacheLayout, Step::HostScopedImports];

    fn version(self) -> &'static str {
        match self {
            Step::LegacyCacheLayout => legacy_cache_layout::VERSION,
            Step::HostScopedImports => host_scoped_imports::VERSION,
        }
    }

    /// The returned detail is empty when there was nothing on disk to change —
    /// the common case, and the reason a healthy startup prints nothing.
    fn run(self, ace: &mut Ace) -> Result<String, MigrateError> {
        match self {
            Step::LegacyCacheLayout => legacy_cache_layout::run(ace),
            Step::HostScopedImports => host_scoped_imports::run(ace),
        }
    }
}

pub struct Migrate;

impl Migrate {
    pub fn run(&self, ace: &mut Ace) -> Result<(), MigrateError> {
        let path = index_toml::index_path()?;
        let (index, rebuilt) = load_or_rebuild(&path)?;
        if let Some(note) = rebuilt {
            ace.warn(&note);
        }
        guard_not_from_the_future(&path, &index.layout_version)?;

        let pending = pending_steps(&index.layout_version);
        if pending.is_empty() {
            return Ok(());
        }

        let mut changed = false;
        for step in pending {
            let detail = step.run(ace)?;
            if !detail.is_empty() {
                ace.done(&format!("Migrated layout to {} ({detail})", step.version()));
                changed = true;
            }
        }

        // A fresh install has no state to stamp: writing the index here would create
        // ACE's data dir on invocations that touch nothing else. The steps above are
        // cheap no-ops until something exists to migrate, and `index_toml::save` stamps
        // the version when setup finally writes the file.
        if !changed && !path.exists() {
            return Ok(());
        }

        // `save` stamps the current layout — steps above may have rewritten the file,
        // so re-read rather than writing back the copy we loaded before they ran. A
        // rebuild here is the same one already announced above.
        let (migrated, _) = load_or_rebuild(&path)?;
        index_toml::save(&path, &migrated)?;
        Ok(())
    }
}

/// `index.toml` holds nothing that cannot be re-derived — the layout stamp and the
/// school entries ACE resolves from `ace.toml` anyway. So an unreadable one is
/// rebuilt rather than treated as fatal; the cost is one re-resolve. A read that
/// fails for any other reason still propagates: state we cannot read *or* rewrite
/// is not something to paper over.
///
/// Returns the note the user should hear, if any. The caller decides whether to say
/// it — the file is read twice per run and a rebuild is one event, not two.
fn load_or_rebuild(path: &std::path::Path) -> Result<(IndexToml, Option<String>), MigrateError> {
    match index_toml::load(path) {
        Err(ConfigError::Parse(e)) => {
            let note = format!("{} is unreadable ({e}); rebuilding it", path.display());
            Ok((IndexToml::default(), Some(note)))
        }
        other => Ok((other?, None)),
    }
}

/// One voice for everything a step declines to delete. Nothing revisits these — the
/// stamp advances either way — so the warning has to name the path and hand the
/// cleanup to the user, or the space is leaked silently.
fn warn_left_behind(ace: &mut Ace, path: &std::path::Path, why: &str) {
    ace.warn(&format!(
        "keeping {} — {why}; delete it manually once you no longer need it",
        path.display(),
    ));
}

/// Refuse state from a newer ACE rather than migrating it downward. Dates compare
/// lexicographically, which is the whole reason the version is an ISO date.
fn guard_not_from_the_future(path: &std::path::Path, found: &str) -> Result<(), MigrateError> {
    if found <= LAYOUT_VERSION {
        return Ok(());
    }

    Err(MigrateError::FromTheFuture {
        path: path.to_path_buf(),
        found: found.to_string(),
    })
}

fn pending_steps(current: &str) -> Vec<Step> {
    Step::ALL
        .into_iter()
        .filter(|s| s.version() > current)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_version_names_the_newest_step() {
        let newest = Step::ALL.last().expect("registry is never empty").version();
        assert_eq!(newest, LAYOUT_VERSION);
    }

    #[test]
    fn steps_are_registered_oldest_first() {
        let versions: Vec<&str> = Step::ALL.iter().map(|s| s.version()).collect();
        let mut sorted = versions.clone();
        sorted.sort_unstable();
        assert_eq!(versions, sorted, "registry must be date-ordered");
    }

    #[test]
    fn one_step_per_date() {
        let mut versions: Vec<&str> = Step::ALL.iter().map(|s| s.version()).collect();
        versions.dedup();
        assert_eq!(versions.len(), Step::ALL.len(), "two steps share a date");
    }

    #[test]
    fn unversioned_state_runs_every_step() {
        assert_eq!(pending_steps(""), Step::ALL.to_vec());
    }

    #[test]
    fn current_state_runs_nothing() {
        assert!(pending_steps(LAYOUT_VERSION).is_empty());
    }

    #[test]
    fn partial_state_runs_only_newer_steps() {
        let first = Step::ALL[0];
        assert_eq!(pending_steps(first.version()), vec![Step::ALL[1]]);
    }

    #[test]
    fn state_from_the_future_is_refused() {
        let path = std::path::Path::new("index.toml");
        assert!(guard_not_from_the_future(path, "9999-01-01").is_err());
        assert!(guard_not_from_the_future(path, LAYOUT_VERSION).is_ok());
        assert!(guard_not_from_the_future(path, "").is_ok());
    }
}
