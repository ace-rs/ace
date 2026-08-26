use crate::ace::{Ace, StartMode};

use super::CmdError;

pub fn run(ace: &mut Ace, backend_args: Vec<String>, mode: StartMode) {
    ace.set_backend_args(backend_args);
    let result = ace.start(mode).map_err(CmdError::from);
    super::exit_on_err(ace, result);
}
