use crate::ace::Ace;
use crate::backend::BackendError;
use crate::config::ConfigError;
use crate::config::paths::ace_data_dir;
use crate::school::SchoolError;
use crate::school::linked::LinkedSchool;

use super::link_skills;
use super::{
    Link, Pull, PullOutcome, RegisterMcpError, SkillChange, UpdateGitignore,
    clone_school::CloneSchool, register_missing_mcp,
};

#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("{0}")]
    Backend(#[from] BackendError),
    #[error("{0}")]
    School(#[from] SchoolError),
    #[error("{0}")]
    RegisterMcp(#[from] RegisterMcpError),
    #[error("clone failed: {0}")]
    Clone(String),
    #[error("write failed: {0}")]
    Write(std::io::Error),
    #[error("skills blocked by leftover links: {}", .0.join(", "))]
    BlockedLinks(Vec<String>),
}

impl PrepareError {
    /// Recovery hint, per `docs/spec/ux.md` §3.
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::BlockedLinks(_) => Some("run `ace link --force` to replace them"),
            Self::Config(_)
            | Self::Backend(_)
            | Self::School(_)
            | Self::RegisterMcp(_)
            | Self::Clone(_)
            | Self::Write(_) => None,
        }
    }
}

/// Ensure school is ready: install if not cached, update if cached, link into project.
pub struct Prepare<'a> {
    pub specifier: &'a str,
}

#[derive(Debug, Default)]
pub struct PrepareResult {
    pub changes: Vec<SkillChange>,
    pub school_is_dirty: bool,
}

// Backend support matrix lives on `Kind::is_folder_supported()`.

impl Prepare<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<PrepareResult, PrepareError> {
        let project_dir = ace.project_dir().to_path_buf();
        let preliminary_backend = ace.backend()?.clone();
        let school = LinkedSchool::resolve(&project_dir, self.specifier)?;

        // Decide install-vs-update by on-disk state, not the index.
        // A stale index entry (clone dir deleted, pre-XDG upgrade, etc.) would
        // otherwise route us into Pull and hit "school not installed".
        let (changes, school_is_dirty, school_updated) = if school.needs_clone() {
            CloneSchool { school: &school }.run(ace)?;
            (Vec::new(), false, true)
        } else {
            let outcome = (Pull {
                school: &school,
                force: false,
            })
            .run(ace)?;
            outcome.emit(ace);
            match outcome {
                PullOutcome::Dirty { .. } => (Vec::new(), true, false),
                PullOutcome::Updated { changes } => (changes, false, true),
                _ => (Vec::new(), false, false),
            }
        };

        // Resolve which skills to link before constructing Link.
        let tree = ace.require_tree()?.clone();
        let prepared = link_skills::prepare(&school.root, &tree, preliminary_backend.features())
            .map_err(PrepareError::Write)?;
        let ace_data_root = ace_data_dir()?;

        let result = Link {
            school_root: &school.root,
            project_dir: &project_dir,
            backend_dir: preliminary_backend.backend_dir(),
            skills: &prepared.desired,
            ace_data_root: &ace_data_root,
            // Only `ace link --force` carries that authorization; every other
            // entry point fails and points the user at it.
            force: link_skills::Force::No,
        }
        .run(ace)?;
        for folder in &result.folders {
            if folder.adopted {
                ace.done(&format!("Moved previous {0} to previous-{0}/", folder.name));
            }
            if folder.linked {
                if preliminary_backend.kind.is_folder_supported(folder.name) {
                    ace.done(&format!("Linked {}", folder.name));
                } else {
                    ace.warn(&format!(
                        "Linked {0}/ — not natively supported by {1} (linked for future compatibility)",
                        folder.name,
                        preliminary_backend.name,
                    ));
                }
            }
        }
        link_skills::emit_warnings(ace, &prepared, &result);

        UpdateGitignore {
            project_dir: &project_dir,
        }
        .run(ace)
        .map_err(PrepareError::Write)?;

        let result = PrepareResult {
            changes,
            school_is_dirty,
        };

        if school_updated {
            ace.invalidate_school_caches();
        }

        let backend = ace.backend()?.clone();
        let raw_entries = match ace.school() {
            Ok(school) => school.mcp.clone(),
            Err(error) if error.is_absent() => Vec::new(),
            Err(error) => return Err(error.into()),
        };
        let excluded = ace.excluded_mcp();
        let entries = raw_entries
            .into_iter()
            .filter(|entry| !excluded.contains(&entry.name))
            .collect::<Vec<_>>();
        let local_path = ace.paths().local.clone();
        register_missing_mcp(ace, &backend, &entries, &project_dir, &local_path)?;

        Ok(result)
    }
}
