mod host_scoped_imports;
mod legacy_cache_layout;

use std::path::PathBuf;

use crate::ace::Ace;
use crate::config::ConfigError;
use crate::config::index_toml::{self, LAYOUT_VERSION};

/// On-disk layout migrations. See `docs/spec/migrations.md`.
///
/// The version ACE last migrated to is recorded as `layout_version` in `index.toml`,
/// dated rather than counted, so a step lines up with the decision doc behind it. Steps
/// run oldest-first at startup, and re-derivable state is torn down and re-fetched rather
/// than transformed in place.
#[derive(Debug, thiserror::Error)]
pub enum MigrateError {
    #[error(
        "{path} was written by a newer ace (layout {found}, this binary knows {LAYOUT_VERSION})"
    )]
    FromTheFuture { path: PathBuf, found: String },
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
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

    /// `Ok(None)` when there was nothing on disk to change — the common case, and the
    /// reason a healthy startup prints nothing.
    fn run(self, ace: &mut Ace) -> Result<Option<String>, MigrateError> {
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
        let index = index_toml::load(&path)?;
        guard_not_from_the_future(&path, &index.layout_version)?;

        let pending = pending_steps(&index.layout_version);
        if pending.is_empty() {
            return Ok(());
        }

        let mut changed = false;
        for step in pending {
            if let Some(detail) = step.run(ace)? {
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
        // so re-read rather than writing back the copy we loaded before they ran.
        let migrated = index_toml::load(&path)?;
        index_toml::save(&path, &migrated)?;
        Ok(())
    }
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
