use std::path::Path;

use crate::ace::Ace;
use crate::backend::Backend;
use crate::config::school_paths;
use crate::config::ConfigError;

use super::link_skills;
use super::{clone, Link, Pull, PullOutcome, SkillChange};

#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("clone failed: {0}")]
    Clone(String),
    #[error("write failed: {0}")]
    Write(std::io::Error),
}

/// Ensure school is ready: install if not cached, update if cached, link into project.
pub struct Prepare<'a> {
    pub specifier: &'a str,
    pub project_dir: &'a Path,
    pub backend: &'a Backend,
}

#[derive(Debug, Default)]
pub struct PrepareResult {
    pub changes: Vec<SkillChange>,
    pub school_is_dirty: bool,
}

// Backend support matrix lives on `Kind::is_folder_supported()`.

impl Prepare<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<PrepareResult, PrepareError> {
        // Decide install-vs-update by on-disk state, not the index.
        // A stale index entry (clone dir deleted, pre-XDG upgrade, etc.) would
        // otherwise route us into Pull and hit "school not installed".
        let school_paths = school_paths::resolve(self.project_dir, self.specifier)?;
        let needs_clone = school_paths
            .clone_path
            .as_ref()
            .is_some_and(|p| !p.join(".git").exists());

        let (changes, school_is_dirty) = if needs_clone {
            clone::Clone {
                specifier: self.specifier,
                project_dir: self.project_dir,
            }
            .run(ace)?;
            (Vec::new(), false)
        } else {
            let outcome = (Pull {
                specifier: self.specifier,
                project_dir: self.project_dir,
                force: false,
            })
            .run(ace)?;
            outcome.emit(ace);
            match outcome {
                PullOutcome::Dirty { .. } => (Vec::new(), true),
                PullOutcome::Updated { changes } => (changes, false),
                _ => (Vec::new(), false),
            }
        };

        // Resolve which skills to link before constructing Link.
        let tree = ace.require_tree()?.clone();
        let prepared = link_skills::prepare(&school_paths.root, &tree)
            .map_err(PrepareError::Write)?;

        let result = Link {
            school_root: &school_paths.root,
            project_dir: self.project_dir,
            backend_dir: self.backend.backend_dir(),
            skills: &prepared.desired,
        }
        .run(ace)?;
        for folder in &result.folders {
            if folder.adopted {
                ace.done(&format!("Moved previous {0} to previous-{0}/", folder.name));
            }
            if folder.linked {
                if self.backend.kind.is_folder_supported(folder.name) {
                    ace.done(&format!("Linked {}", folder.name));
                } else {
                    ace.warn(&format!(
                        "Linked {0}/ — not natively supported by {1} (linked for future compatibility)",
                        folder.name,
                        self.backend.name,
                    ));
                }
            }
        }
        link_skills::emit_warnings(ace, &prepared, &result);

        Ok(PrepareResult {
            changes,
            school_is_dirty,
        })
    }
}
