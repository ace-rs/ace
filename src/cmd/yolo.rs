use crate::ace::Ace;
use crate::config::Scope;
use crate::config::ace_toml::{self, Trust};

use super::CmdError;

pub fn run(ace: &mut Ace, trust: Trust) {
    let result = run_inner(ace, trust);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, trust: Trust) -> Result<(), CmdError> {
    let scope = ace.scope_override().unwrap_or(Scope::Local);
    let paths = ace.paths();
    let target = scope.path_in(paths);

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut config = ace_toml::load_or_default(target)?;
    config.trust = trust;
    config.yolo = false; // clear deprecated field
    ace_toml::save(target, &config)?;

    let msg = match trust {
        Trust::Auto => "Auto mode — AI decides which actions need approval",
        Trust::Yolo => "Yolo mode — all permission prompts disabled",
        Trust::Default => "Default mode — using backend's standard permission handling",
    };
    ace.done(msg);
    Ok(())
}
