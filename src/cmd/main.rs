use crate::ace::Ace;
use crate::actions::project::{Prepare, PrepareResult, register_missing_mcp};
use crate::backend::{OneShotRequest, PromptInput, SessionRequest};
use crate::config::ace_toml::Trust;
use crate::config::resolve::Source;
use crate::school::SchoolError;
use crate::school::linked::LinkedSchool;
use crate::templates::session::{SessionPromptInput, build_session_prompt};

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
    require_config_or_recover(ace)?;

    let (specifier, school_from) = {
        let r = ace.require_config()?;
        let specifier = r
            .school_specifier
            .value
            .clone()
            .ok_or(SchoolError::NoSpecifier)?;
        (specifier, r.school_specifier.from)
    };

    if let Some(notice) = school_source_notice(school_from, &specifier) {
        ace.info(&notice);
    }

    let prepare_result = prepare_school(ace, &specifier)?;

    let project_dir = ace.project_dir().to_path_buf();
    let school_clone = ace.require_linked_school()?.clone_path.clone();

    let (school_name, school_session_prompt) = {
        let school = ace.school()?.ok_or(SchoolError::NoSpecifier)?;
        (school.name.clone(), school.session_prompt.clone())
    };

    let backend_dir = project_dir.join(ace.backend()?.backend_dir());

    let (resolved_session_prompt, trust, resume_pref, env) = {
        let r = ace.require_config()?;
        let env: std::collections::HashMap<String, String> = r
            .env
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();
        (
            r.session_prompt.value.clone(),
            r.trust.value,
            r.resume.value,
            env,
        )
    };

    let excluded_skills = ace.excluded_skills();
    let session_prompt = build_session_prompt(&SessionPromptInput {
        school_name: &school_name,
        school_session_prompt: &school_session_prompt,
        project_session_prompt: &resolved_session_prompt,
        backend_dir: &backend_dir,
        changes: &prepare_result.changes,
        school_clone: school_clone.as_deref(),
        school_is_dirty: prepare_result.school_is_dirty,
        excluded_skills: &excluded_skills,
    });

    let kind = ace.backend()?.kind;
    if kind.supports_trust(trust) {
        match trust {
            Trust::Auto => ace.info("auto mode — AI decides approvals"),
            Trust::Yolo => ace.warn("yolo mode — permission prompts disabled"),
            Trust::Default => {}
        }
    } else {
        ace.warn(&format!(
            "{} does not support {} trust — running with its default permissions",
            kind.name(),
            trust.label(),
        ));
    }

    let resume = should_resume && resume_pref;

    if let Some(prompt) = one_shot_prompt {
        // One-shot path: spawn-and-capture, print captured output, propagate exit code.
        // No resume hint, no separator — output should be clean for piping.
        let output = ace.backend()?.exec_one_shot(OneShotRequest {
            prompt: PromptInput::Inline(prompt),
            project_dir,
            env,
            extra_args: backend_args,
        })?;

        use std::io::Write;
        let _ = std::io::stdout().write_all(&output.stdout);
        let _ = std::io::stderr().write_all(&output.stderr);

        if !output.status.success() {
            std::process::exit(output.status.code().unwrap_or(1));
        }
        return Ok(());
    }

    if resume {
        ace.hint("Resuming previous session. If this fails, run: ace new");
    }

    ace.separator();

    ace.backend()?.exec_session(SessionRequest {
        trust,
        session_prompt,
        project_dir,
        env,
        extra_args: backend_args,
        resume,
    })?;

    Ok(())
}

/// Shared workflow: prepare school (install/update/link) → register MCP servers.
///
/// Called by both bare `ace` and `ace setup`. Reloads state after linking so
/// school.toml is available for MCP registration and downstream callers.
pub(super) fn prepare_school(ace: &mut Ace, specifier: &str) -> Result<PrepareResult, CmdError> {
    let project_dir = ace.project_dir().to_path_buf();
    let preliminary_backend = ace.backend()?.clone();

    // Paths-only resolution — first run has no school.toml yet, so the
    // content-checked `require_linked_school` would refuse the very state
    // Prepare exists to heal.
    let school = LinkedSchool::resolve(&project_dir, specifier)?;
    let prepare_result = (Prepare {
        school: &school,
        project_dir: &project_dir,
        backend: &preliminary_backend,
    })
    .run(ace)?;

    // Reload with fresh school.toml when Prepare cloned or pulled changes;
    // a no-op prepare leaves the already-loaded caches valid.
    if prepare_result.school_updated {
        ace.invalidate_school_caches();
    }

    // Register MCP servers from school.toml.
    let (backend, entries, _) = super::mcp::load_school_mcp(ace)?;
    if entries.is_empty() {
        return Ok(prepare_result);
    }

    let local_path = ace.paths().local.clone();
    if let Err(e) = register_missing_mcp(ace, &backend, &entries, &project_dir, &local_path) {
        ace.warn(&format!("MCP registration failed: {e}"));
    }

    Ok(prepare_result)
}

/// Try resolving the backend binding. On unknown backend in TTY mode, prompt
/// the user to pick a known backend, set it as a runtime override, and retry.
/// Closes PROD9-146: a stale `backend = "..."` selector can no longer brick
/// the session — the user gets a recovery prompt instead.
fn require_config_or_recover(ace: &mut Ace) -> Result<(), CmdError> {
    ace.require_config()?;
    match ace.backend() {
        Ok(_) => Ok(()),
        Err(crate::backend::BackendError::Unknown(name)) => recover_backend(ace, &name),
        Err(e) => Err(e.into()),
    }
}

fn recover_backend(ace: &mut Ace, attempted: &str) -> Result<(), CmdError> {
    if !ace.can_ask() {
        ace.hint(&format!(
            "to fix: ace config set backend <name> (registry has no `{attempted}`)"
        ));
        return Err(crate::backend::BackendError::Unknown(attempted.to_string()).into());
    }

    let names = ace.known_backend_names()?;
    ace.warn(&format!("backend `{attempted}` is not in the registry"));
    let pick = ace.prompt_select("Pick a backend for this session:", names)?;
    ace.override_backend(pick.clone());
    ace.backend()?;
    ace.hint(&format!("to make permanent: ace config set backend {pick}"));
    Ok(())
}

/// Announce a school that did not come from this repo's `ace.toml`. The
/// project layer is the unremarkable case, and an override was just typed by
/// hand — neither is worth a line.
fn school_source_notice(from: Source, specifier: &str) -> Option<String> {
    match from {
        Source::User => Some(format!("school {specifier} — user config")),
        Source::Local => Some(format!("school {specifier} — local config")),
        Source::Project | Source::Override | Source::School | Source::Default => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_school_is_announced() {
        let notice = school_source_notice(Source::User, "ace-rs/school");
        assert_eq!(
            notice.as_deref(),
            Some("school ace-rs/school — user config")
        );
    }

    #[test]
    fn local_school_is_announced() {
        let notice = school_source_notice(Source::Local, "ace-rs/school");
        assert_eq!(
            notice.as_deref(),
            Some("school ace-rs/school — local config")
        );
    }

    #[test]
    fn project_school_is_silent() {
        assert!(school_source_notice(Source::Project, "ace-rs/school").is_none());
    }

    #[test]
    fn typed_override_is_silent() {
        assert!(school_source_notice(Source::Override, "ace-rs/school").is_none());
    }
}
