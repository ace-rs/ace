use crate::ace::{Ace, OutputMode};
use crate::backend::{Kind, SessionOpts};
use crate::config::ace_toml::Trust;
use crate::actions::project::RegisterMcp;
use crate::actions::project::{Prepare, PrepareResult};
use crate::templates::session::build_session_prompt;

use super::CmdError;

pub fn run(
    ace: &mut Ace,
    backend_args: Vec<String>,
    should_resume: bool,
    one_shot_prompt: Option<String>,
) {
    let result = run_inner(ace, backend_args, should_resume, one_shot_prompt);
    super::exit_on_err(ace, result);
}

fn run_inner(
    ace: &mut Ace,
    backend_args: Vec<String>,
    should_resume: bool,
    one_shot_prompt: Option<String>,
) -> Result<(), CmdError> {
    require_resolved_or_recover(ace)?;

    let specifier = {
        let r = ace.require_resolved()?;
        r.school_specifier.value.clone().ok_or(crate::school::SchoolError::NoSpecifier)?
    };

    let prepare_result = prepare_school(ace, &specifier)?;

    let project_dir = ace.project_dir().to_path_buf();
    let school_clone = ace.require_school()?.clone_path.clone();

    let (school_name, school_session_prompt) = {
        let school = ace.school()?.ok_or(crate::school::SchoolError::NoSpecifier)?;
        (school.name.clone(), school.session_prompt.clone())
    };

    let backend_dir = project_dir.join(ace.backend()?.backend_dir());

    let (resolved_session_prompt, trust, resume_pref, env) = {
        let r = ace.require_resolved()?;
        let env: std::collections::HashMap<String, String> = r
            .env
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();
        (r.session_prompt.value.clone(), r.trust.value, r.resume.value, env)
    };

    let session_prompt = build_session_prompt(
        &school_name,
        &school_session_prompt,
        &resolved_session_prompt,
        &backend_dir,
        &prepare_result.changes,
        school_clone.as_deref(),
        prepare_result.school_is_dirty,
    );

    match trust {
        Trust::Auto => ace.hint("auto mode — AI decides approvals"),
        Trust::Yolo => ace.warn("yolo mode — permission prompts disabled"),
        Trust::Default => {}
    }

    let resume = should_resume && resume_pref;
    if resume {
        ace.hint("Resuming previous session. If this fails, run: ace new");
    }

    ace.separator();

    ace.backend()?.exec_session(SessionOpts {
        trust,
        session_prompt,
        project_dir,
        env,
        extra_args: backend_args,
        resume,
        one_shot_prompt,
    })?;

    Ok(())
}

/// Shared workflow: prepare school (install/update/link) → register MCP servers.
///
/// Called by both bare `ace` and `ace setup`. Reloads state after linking so
/// school.toml is available for MCP registration and downstream callers.
pub(super) fn prepare_school(
    ace: &mut Ace,
    specifier: &str,
) -> Result<PrepareResult, CmdError> {
    let project_dir = ace.project_dir().to_path_buf();
    let preliminary_backend = ace.backend()?.clone();

    let prepare_result = (Prepare {
        specifier,
        project_dir: &project_dir,
        backend: &preliminary_backend,
    })
    .run(ace)?;

    // Reload with fresh school.toml after Prepare.
    ace.reload_tree()?;

    // Register MCP servers from school.toml.
    let mcp_entries: Vec<_> = ace.school()?
        .map(|s| s.mcp.clone())
        .unwrap_or_default();

    if mcp_entries.is_empty() {
        return Ok(prepare_result);
    }

    let registered = ace.backend()?.mcp_list();
    let pending: Vec<&str> = mcp_entries.iter()
        .filter(|e| !registered.contains(&e.name))
        .map(|e| e.name.as_str())
        .collect();

    if pending.is_empty() {
        return Ok(prepare_result);
    }

    let prompt = format!("Register MCP server(s): {}?", pending.join(", "));
    if !ace.prompt_confirm(&prompt, true)? {
        return Ok(prepare_result);
    }

    let backend = ace.backend()?.clone();
    if let Err(e) = (RegisterMcp { backend: &backend, entries: &mcp_entries }).run(ace) {
        ace.warn(&format!("MCP registration failed: {e}"));
    }

    Ok(prepare_result)
}

/// Try resolving the backend binding. On unknown backend in TTY mode, prompt
/// the user to pick a known backend, set it as a runtime override, and retry.
/// Closes PROD9-146: a stale `backend = "..."` selector can no longer brick
/// the session — the user gets a recovery prompt instead.
fn require_resolved_or_recover(ace: &mut Ace) -> Result<(), CmdError> {
    ace.require_resolved()?;
    match ace.backend() {
        Ok(_) => Ok(()),
        Err(crate::backend::BackendError::Unknown(name)) => recover_backend(ace, &name),
        Err(e) => Err(e.into()),
    }
}

fn recover_backend(ace: &mut Ace, attempted: &str) -> Result<(), CmdError> {
    if ace.mode() != OutputMode::Human {
        ace.hint(&format!(
            "to fix: ace config set backend <name> (registry has no `{attempted}`)"
        ));
        return Err(crate::backend::BackendError::Unknown(attempted.to_string()).into());
    }

    let names = list_known_backend_names(ace)?;
    ace.warn(&format!("backend `{attempted}` is not in the registry"));
    let pick = ace.prompt_select("Pick a backend for this session:", names)?;
    ace.override_backend(pick.clone());
    ace.backend()?;
    ace.hint(&format!("to make permanent: ace config set backend {pick}"));
    Ok(())
}

fn list_known_backend_names(ace: &Ace) -> Result<Vec<String>, CmdError> {
    let tree = ace.require_tree()?;
    let mut names: Vec<String> = Kind::ALL
        .iter()
        .map(|k| k.name().to_string())
        .collect();
    if let Some(st) = &tree.school {
        names.extend(st.backends.iter().map(|d| d.name.clone()));
    }
    for layer in [&tree.user, &tree.project, &tree.local].iter().filter_map(|o| o.as_ref()) {
        names.extend(layer.backends.iter().map(|d| d.name.clone()));
    }
    names.sort();
    names.dedup();
    Ok(names)
}
