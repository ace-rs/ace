use crate::ace::Ace;
use crate::actions::project::link_skills;
use crate::actions::project::pull::PullOutcome;
use crate::actions::project::{clone, Link, Pull};
use crate::config::school_paths;

use super::CmdError;

pub fn run(ace: &mut Ace) {
    let result = run_inner(ace);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace) -> Result<(), CmdError> {
    let specifier = ace.require_resolved()?
        .school_specifier
        .value
        .clone()
        .ok_or(crate::school::SchoolError::NoSpecifier)?;

    let project_dir = ace.project_dir().to_path_buf();
    let school_paths = school_paths::resolve(&project_dir, &specifier)?;

    // Self-heal: if the clone dir is gone (stale index, deleted cache, etc.),
    // clone instead of pulling — Pull would otherwise error "school not installed".
    let needs_clone = school_paths
        .clone_path
        .as_ref()
        .is_some_and(|p| !p.join(".git").exists());

    if needs_clone {
        clone::Clone {
            specifier: &specifier,
            project_dir: &project_dir,
        }
        .run(ace)?;
    } else {
        let outcome = (Pull {
            specifier: &specifier,
            project_dir: &project_dir,
            force: true,
        })
        .run(ace)?;
        outcome.emit(ace);

        if let PullOutcome::Updated { changes } = &outcome {
            crate::school::skill_count::maybe_hint_relearn(ace, changes);
        }
    }

    // Re-link in case new folders appeared.
    let backend_dir = ace.backend()?.backend_dir();
    let tree = ace.require_tree()?.clone();
    let prepared = link_skills::prepare(&school_paths.root, &tree)
        .map_err(|e| CmdError::Other(format!("scan school skills: {e}")))?;

    let result = Link {
        school_root: &school_paths.root,
        project_dir: &project_dir,
        backend_dir,
        skills: &prepared.desired,
    }
    .run(ace)?;
    link_skills::emit_warnings(ace, &prepared, &result);

    Ok(())
}
