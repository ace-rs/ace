use crate::ace::Ace;
use crate::actions::project::edit_config::{EditConfig, FieldEdit};
use crate::config::Scope;
use crate::config::ace_toml::Trust;

use super::CmdError;

pub fn run(ace: &mut Ace, trust: Trust) {
    let result = run_inner(ace, trust);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, trust: Trust) -> Result<(), CmdError> {
    let scope = ace.scope_override().unwrap_or(Scope::Local);
    let target = scope.path_in(ace.paths()).to_path_buf();
    EditConfig {
        path: &target,
        assignments: vec![
            FieldEdit::new("trust", trust.label()),
            FieldEdit::remove("yolo"),
        ],
    }
    .run(ace)?;

    let msg = match trust {
        Trust::Auto => "Auto mode — AI decides which actions need approval",
        Trust::Yolo => "Yolo mode — all permission prompts disabled",
        Trust::Default => "Default mode — using backend's standard permission handling",
    };
    ace.done(msg);
    Ok(())
}
