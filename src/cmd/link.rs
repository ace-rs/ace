use crate::ace::Ace;
use crate::actions::project::Link;
use crate::actions::project::link_skills;
use crate::config::paths::ace_data_dir;
use crate::config::school_paths;

use super::CmdError;

pub fn run(ace: &mut Ace) {
    let result = run_inner(ace);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace) -> Result<(), CmdError> {
    let specifier = ace
        .require_resolved()?
        .school_specifier
        .value
        .clone()
        .ok_or(crate::school::SchoolError::NoSpecifier)?;

    let project_dir = ace.project_dir().to_path_buf();
    let school_paths = school_paths::resolve(&project_dir, &specifier)?;

    let backend = ace.backend()?;
    let backend_dir = backend.backend_dir();
    let backend_features = backend.features();
    let tree = ace.require_tree()?.clone();
    let prepared = link_skills::prepare(&school_paths.root, &tree, backend_features)
        .map_err(|e| CmdError::failed(format!("scan school skills: {e}")))?;
    let ace_data_root = ace_data_dir()?;

    let result = Link {
        school_root: &school_paths.root,
        project_dir: &project_dir,
        backend_dir,
        skills: &prepared.desired,
        ace_data_root: &ace_data_root,
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
