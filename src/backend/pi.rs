mod actions;
mod arguments;
mod packages;

pub(super) use arguments::OutputMode;

use std::path::{Path, PathBuf};
use std::process::Output;

use super::{Invocation, OneShotRequest, PreflightError, SessionRequest};
use crate::config::ace_toml::Trust;
use crate::session::ResumeMode;

pub(super) fn launch_options(context: &Invocation) -> Result<arguments::Options, PreflightError> {
    arguments::parse(context)
}

pub(super) fn preflight(context: &Invocation) -> Result<(), PreflightError> {
    let packages = actions::ListPackages.run(context)?;
    match packages::installation(&packages) {
        packages::Installation::Installed => Ok(()),
        packages::Installation::Missing => Err(PreflightError::Unavailable(format!(
            "Pi requires a user-scope pi-mcp-adapter installation, but it was not found. Project-only installations are not verified by ACE. Run `{}` and retry ACE. If already installed, use `{} config` to inspect package filtering.",
            recovery_command(context, "install npm:pi-mcp-adapter@3")?,
            recovery_command(context, "")?.trim(),
        ))),
        packages::Installation::UnrecognizedSource => Err(PreflightError::Failed(format!(
            "Cannot verify pi-mcp-adapter installation: `pi list` contains Git or local package sources; ACE currently recognizes the npm installation only. Inspect those sources, or install with `{}` and retry ACE.",
            recovery_command(context, "install npm:pi-mcp-adapter@3")?,
        ))),
    }
}

pub(super) fn installed_package_paths(
    context: &Invocation,
) -> Result<Vec<PathBuf>, PreflightError> {
    actions::ListPackages.run(context).map(|packages| {
        packages
            .into_iter()
            .filter_map(|package| package.installed_path)
            .collect()
    })
}

pub(super) fn exec_session(
    command: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    request: SessionRequest,
) -> Result<(), std::io::Error> {
    actions::LaunchSession {
        command,
        model,
        effort,
    }
    .run(&request)
}

pub(super) fn exec_one_shot(
    command: &[String],
    model: Option<&str>,
    effort: Option<&str>,
    request: OneShotRequest,
) -> Result<Output, std::io::Error> {
    actions::RunOneShot {
        command,
        model,
        effort,
    }
    .run(&request)
}

pub(super) fn supports_trust(trust: Trust) -> bool {
    matches!(trust, Trust::Default)
}

pub(super) fn is_ready() -> bool {
    // Authentication is provider-specific; filesystem presence cannot establish it.
    false
}

fn build_session_args(
    model: Option<&str>,
    effort: Option<&str>,
    request: &SessionRequest,
    prompt_path: Option<&Path>,
) -> std::io::Result<Vec<String>> {
    let mut args = Vec::new();
    match request.resume {
        ResumeMode::Fresh => {
            let path = prompt_path.and_then(Path::to_str).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Pi instructions require a UTF-8 prompt file path",
                )
            })?;
            args.extend(["--append-system-prompt".into(), path.into()]);
        }
        ResumeMode::Latest => args.push("--continue".into()),
    }
    args.extend(model_args(model, effort));
    args.extend(request.extra_args.iter().cloned());
    Ok(args)
}

pub(super) fn build_one_shot_args(
    model: Option<&str>,
    effort: Option<&str>,
    request: &OneShotRequest,
) -> Vec<String> {
    let mut args = vec!["--print".into()];
    args.extend(model_args(model, effort));
    args.extend(request.extra_args.iter().cloned());
    args
}

fn model_args(model: Option<&str>, effort: Option<&str>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(model) = model {
        args.extend(["--model".into(), model.into()]);
    }
    if let Some(effort) = effort {
        args.extend(["--thinking".into(), effort.into()]);
    }
    args
}

fn recovery_command(context: &Invocation, operation: &str) -> Result<String, PreflightError> {
    let options = launch_options(context)?;
    let command = options
        .probe_command
        .iter()
        .map(|word| shell_word(word))
        .collect::<Vec<_>>()
        .join(" ");
    let directory = agent_dir(context)?;
    let directory = directory.to_str().ok_or_else(|| {
        PreflightError::InvalidArguments("Pi agent directory is not valid UTF-8".into())
    })?;

    Ok(format!(
        "PI_CODING_AGENT_DIR={} {command} {operation}",
        shell_word(directory)
    ))
}

/// Pi's documented agent directory, shared by provisioning and recovery hints.
pub(super) fn agent_dir(context: &Invocation) -> Result<PathBuf, PreflightError> {
    let home = context
        .home_dir()
        .map_err(PreflightError::InvalidArguments)?;
    let Some(configured) = context.env_value("PI_CODING_AGENT_DIR") else {
        return Ok(home.join(".pi/agent"));
    };
    let configured = configured
        .to_str()
        .ok_or_else(|| {
            PreflightError::InvalidArguments("PI_CODING_AGENT_DIR is not valid UTF-8".into())
        })?
        .trim();
    Ok(match configured {
        "" => home.join(".pi/agent"),
        "~" => home,
        value if value.starts_with("~/") => home.join(&value[2..]),
        value => context.project_dir.join(value),
    })
}

fn shell_word(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || "_./:@=-".contains(ch))
    {
        return value.into();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn session() -> SessionRequest {
        SessionRequest {
            trust: Trust::Default,
            session_prompt: "literal instructions".into(),
            project_dir: PathBuf::from("/project"),
            env: HashMap::new(),
            extra_args: Vec::new(),
            resume: ResumeMode::Fresh,
        }
    }

    #[test]
    fn appends_instruction_file_and_preserves_passthrough_precedence() {
        let mut request = session();
        request.extra_args = vec!["--model".into(), "override".into()];
        let args = build_session_args(
            Some("provider/model"),
            Some("high"),
            &request,
            Some(Path::new("/tmp/prompt")),
        )
        .unwrap();
        assert_eq!(
            args,
            [
                "--append-system-prompt",
                "/tmp/prompt",
                "--model",
                "provider/model",
                "--thinking",
                "high",
                "--model",
                "override"
            ]
        );
    }

    #[test]
    fn resumes_without_replacing_session_instructions() {
        let mut request = session();
        request.resume = ResumeMode::Latest;
        assert_eq!(
            build_session_args(None, None, &request, None).unwrap(),
            ["--continue"]
        );
    }

    #[test]
    fn one_shot_prompt_never_becomes_an_option_or_attachment() {
        for text in ["@private.txt", "--help", "/path/to/existing/file"] {
            let request = OneShotRequest {
                prompt: super::super::PromptInput::Inline(text.into()),
                project_dir: PathBuf::from("/project"),
                env: HashMap::new(),
                extra_args: vec!["--thinking".into(), "low".into()],
            };
            let args = build_one_shot_args(Some("provider/model"), Some("high"), &request);
            assert_eq!(
                args,
                [
                    "--print",
                    "--model",
                    "provider/model",
                    "--thinking",
                    "high",
                    "--thinking",
                    "low"
                ]
            );
        }
    }

    #[test]
    fn rejects_extension_disabling_flags_but_not_literal_values() {
        let mut context = Invocation {
            kind: super::super::Kind::Pi,
            command: vec!["pi".into()],
            project_dir: PathBuf::from("/project"),
            env: HashMap::new(),
            model: None,
            effort: None,
            extra_args: vec!["--no-extensions".into()],
        };
        assert!(matches!(
            launch_options(&context),
            Err(PreflightError::InvalidArguments(_))
        ));
        context.extra_args = vec!["--model".into(), "--no-extensions".into()];
        assert!(launch_options(&context).is_ok());
        context.extra_args = vec!["--".into(), "--no-extensions".into()];
        assert!(launch_options(&context).is_ok());
        context.command.push("-ne".into());
        assert!(launch_options(&context).is_err());
    }

    #[test]
    fn prevents_command_prefix_from_turning_inventory_into_a_launch_or_install() {
        for prefix in [
            "--print",
            "-p",
            "--mode",
            "--provider",
            "--model",
            "--continue",
            "install",
            "update",
            "list",
        ] {
            let context = Invocation {
                kind: super::super::Kind::Pi,
                command: vec!["pi".into(), prefix.into()],
                project_dir: PathBuf::from("/project"),
                env: HashMap::new(),
                model: None,
                effort: None,
                extra_args: Vec::new(),
            };
            assert!(launch_options(&context).is_err(), "{prefix}");
        }
    }
}
