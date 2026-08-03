use crate::ace::Ace;
use crate::actions::project::Link;
use crate::actions::project::link_skills;
use crate::config::paths::ace_data_dir;

use super::CmdError;

pub fn run(ace: &mut Ace, force: bool) {
    let result = run_inner(ace, force);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, force: bool) -> Result<(), CmdError> {
    let school_root = ace.require_linked_school()?.root.clone();
    let project_dir = ace.project_dir().to_path_buf();

    let backend = ace.backend()?;
    let backend_dir = backend.backend_dir();
    let backend_features = backend.features();
    let tree = ace.require_tree()?.clone();
    let prepared = link_skills::prepare(&school_root, &tree, backend_features)
        .map_err(|e| CmdError::failed(format!("scan school skills: {e}")))?;
    let ace_data_root = ace_data_dir()?;

    let result = Link {
        school_root: &school_root,
        project_dir: &project_dir,
        backend_dir,
        skills: &prepared.desired,
        ace_data_root: &ace_data_root,
        force: match force {
            true => link_skills::Force::Yes,
            false => link_skills::Force::No,
        },
    }
    .run(ace)?;

    let mut any_linked = false;
    for folder in &result.folders {
        if folder.linked {
            ace.done(&format!("Linked {}", folder.name));
            any_linked = true;
        }
    }
    if !any_linked && !result.folders.is_empty() {
        ace.info("All school folders already linked.");
    }
    link_skills::emit_warnings(ace, &prepared, &result);

    Ok(())
}
