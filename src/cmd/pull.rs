use crate::ace::Ace;
use crate::actions::project::{Pull, clone};
use crate::school::linked::LinkedSchool;

use super::CmdError;

pub fn run(ace: &mut Ace) {
    let result = run_inner(ace);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace) -> Result<(), CmdError> {
    let specifier = ace
        .require_config()?
        .school_specifier
        .value
        .clone()
        .ok_or(crate::school::SchoolError::NoSpecifier)?;

    // Paths-only resolution — self-heal must reach states the content-checked
    // `require_linked_school` refuses (missing or uninitialized clone).
    let school = LinkedSchool::resolve(ace.project_dir(), &specifier)?;

    // Self-heal: if the clone dir is gone (stale index, deleted cache, etc.),
    // clone instead of pulling — Pull would otherwise error "school not installed".
    if school.needs_clone() {
        clone::Clone { school: &school }.run(ace)?;
    } else {
        let outcome = (Pull {
            school: &school,
            force: true,
        })
        .run(ace)?;
        outcome.emit(ace);
    }

    Ok(())
}
