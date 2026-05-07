use crate::ace::{Ace, OutputMode};
use crate::actions::project::LearnAction;

use super::CmdError;

pub fn run(ace: &mut Ace) {
    let result = run_inner(ace);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace) -> Result<(), CmdError> {
    // Human mode: prompt before spending tokens. Porcelain/non-Human:
    // explicit `ace learn` invocation IS consent — proceed directly.
    if ace.mode() == OutputMode::Human {
        let prompt = "ace learn spawns the backend and spends tokens. Proceed?";
        if !ace.prompt_confirm(prompt, false)? {
            return Ok(());
        }
    }

    LearnAction.run(ace)?;
    Ok(())
}
