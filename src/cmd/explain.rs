use crate::ace::Ace;
use crate::actions::project::explain_skill::{find_or_suggest, render};

use super::CmdError;

pub fn run(ace: &mut Ace, name: &str) {
    let result = run_inner(ace, name);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, name: &str) -> Result<(), CmdError> {
    ace.require_config()?;
    let rendered = {
        let skills = ace.skills()?;
        let skill = find_or_suggest(skills, name).map_err(|e| CmdError::usage(e.to_string()))?;
        render(skill)
    };
    ace.data(&rendered);
    Ok(())
}
