use crate::ace::Ace;
use crate::config::index_toml;
use crate::git;
use crate::actions::project::Setup;
use crate::templates;

use super::CmdError;

pub fn run(ace: &mut Ace, specifier: Option<&str>) {
    let result = run_inner(ace, specifier);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, specifier: Option<&str>) -> Result<(), CmdError> {
    let project_dir = ace.project_dir().to_path_buf();

    let resolved = match specifier {
        Some(s) => normalize_specifier(s),
        None => resolve_from_cache(ace)?,
    };

    Setup {
        specifier: &resolved,
        project_dir: &project_dir,
    }
    .run(ace)?;

    // Prepare school (install/update/link + MCP).
    ace.require_resolved()?;
    super::main::prepare_school(ace, &resolved)?;

    // Post-prepare setup: instructions file. Gitignore refresh runs inside Prepare.
    let backend = ace.backend()?.clone();

    let instructions = project_dir.join(backend.instructions_file());
    if !instructions.exists() {
        let school_name = ace.school()?
            .map(|s| s.name.clone())
            .unwrap_or_default();

        let backend_dir_name = backend.backend_dir();
        let tpl = templates::Template::parse(templates::builtins::PROJECT_CLAUDE_MD);
        let content = tpl.substitute(&std::collections::HashMap::from([
            ("school_name".to_string(), school_name),
            ("backend_dir".to_string(), backend_dir_name.to_string()),
        ]));

        std::fs::write(&instructions, content)?;
        ace.done(&format!("Created {}", backend.instructions_file()));
    }

    // Skill-count check: if school exposes more than the threshold and the
    // user hasn't explicitly set `skills` in ace.toml, offer to run learn.
    crate::school::skill_count::maybe_offer_learn(ace)?;

    ace.done("Setup complete.");
    Ok(())
}

/// Normalize a specifier: strip GitHub URL prefix and .git suffix from the source portion.
/// Preserves colon-separated path (e.g. `owner/repo:subpath`).
fn normalize_specifier(spec: &str) -> String {
    match spec.split_once(':') {
        Some((source, path)) => {
            let normalized = git::normalize_github_source(source);
            format!("{normalized}:{path}")
        }
        None => git::normalize_github_source(spec),
    }
}

fn resolve_from_cache(ace: &mut Ace) -> Result<String, CmdError> {
    let index_path = index_toml::index_path()
        .map_err(|e| CmdError::Other(format!("{e}")))?;
    let legacy_path = index_toml::legacy_index_path()
        .map_err(|e| CmdError::Other(format!("{e}")))?;
    let index = index_toml::load_or_migrate(&index_path, &legacy_path)
        .map_err(|e| CmdError::Other(format!("{e}")))?;

    let specs = index_toml::list_specifiers(&index);
    match specs.len() {
        0 => Err(CmdError::Other("no cached schools, ace setup <owner/repo>?".to_string())),
        1 => Ok(specs.into_iter().next().expect("checked len=1")),
        _ => {
            let choice = ace.prompt_select("Select school:", specs)?;
            Ok(choice)
        }
    }
}
