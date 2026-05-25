use std::path::Path;

use crate::ace::Ace;
use crate::config;

use super::CmdError;

pub fn run(ace: &mut Ace) {
    let result = run_inner(ace);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace) -> Result<(), CmdError> {
    let cwd = ace.project_dir().to_path_buf();
    let mut formatted = 0;

    let ace_path = cwd.join("ace.toml");
    if ace_path.exists() {
        format_ace_toml(ace, &ace_path)?;
        formatted += 1;
    }

    let school_path = cwd.join("school.toml");
    if school_path.exists() {
        format_school_toml(ace, &school_path)?;
        formatted += 1;
    }

    if formatted == 0 {
        ace.info("no ace.toml or school.toml in current directory — nothing to format");
    }

    Ok(())
}

fn format_ace_toml(ace: &mut Ace, path: &Path) -> Result<(), CmdError> {
    let toml = config::ace_toml::load(path)?;
    config::ace_toml::save(path, &toml)?;
    ace.done(&format!("Formatted {}", path.display()));
    Ok(())
}

fn format_school_toml(ace: &mut Ace, path: &Path) -> Result<(), CmdError> {
    let toml = config::school_toml::load(path)?;
    config::school_toml::save(path, &toml)?;
    ace.done(&format!("Formatted {}", path.display()));
    Ok(())
}
