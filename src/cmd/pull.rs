use crate::ace::Ace;
use crate::actions::project::{Pull, clone_school::CloneSchool};
use crate::school::linked::{self, LinkedSchool};

use super::CmdError;

/// `ace pull [specifier]` — clone or update a school as an ACE-managed clone. A named
/// school needs no project config; without one, the linked school is pulled.
pub fn run(ace: &mut Ace, specifier: Option<&str>) {
    let result = run_inner(ace, specifier);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, specifier: Option<&str>) -> Result<(), CmdError> {
    let specifier = match specifier {
        Some(named) => linked::normalize_specifier(named),
        None => linked_specifier(ace)?,
    };

    // Paths-only resolution — self-heal must reach states the content-checked
    // `require_linked_school` refuses (missing or uninitialized clone).
    let school = LinkedSchool::resolve(ace.project_dir(), &specifier)?;

    // Self-heal: if the clone dir is gone (stale index, deleted cache, etc.),
    // clone instead of pulling — Pull would otherwise error "school not installed".
    if school.needs_clone() {
        CloneSchool { school: &school }.run(ace)?;
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

fn linked_specifier(ace: &mut Ace) -> Result<String, CmdError> {
    let specifier = ace.require_config()?.school_specifier.value.clone();
    Ok(specifier.ok_or(crate::school::SchoolError::NoSpecifier)?)
}
