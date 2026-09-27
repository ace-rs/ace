use std::io;
use std::process::{Command, Output, Stdio};

use super::super::{Invocation, OneShotRequest, PreflightError, PromptInput, SessionRequest};
use super::{build_one_shot_args, build_session_args, launch_options, packages, recovery_command};
use crate::session::{ResumeMode, SessionProcess, StorePrompt};

pub(super) struct ListPackages;

impl ListPackages {
    pub(super) fn run(
        &self,
        context: &Invocation,
    ) -> Result<Vec<packages::Package>, PreflightError> {
        let options = launch_options(context)?;
        let mut probe = context.clone();
        probe.command = options.probe_command;
        let diagnostic_command = recovery_command(context, "list --no-approve")?;
        let output = probe
            .process()
            .args(["list", "--no-approve"])
            .env("NO_COLOR", "1")
            .env("PI_OFFLINE", "1")
            .stdin(Stdio::null())
            .output()
            .map_err(|error| probe_error(error, &diagnostic_command))?;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if !output.status.success() || !stderr.trim().is_empty() {
            return Err(PreflightError::Failed(format!(
                "Cannot verify pi-mcp-adapter installation: `{diagnostic_command}` exited \
                 with {}. Resolve the reported diagnostics and retry ACE.\n{}\n{}",
                output.status,
                stderr.trim(),
                stdout.trim(),
            )));
        }
        let stdout = std::str::from_utf8(&output.stdout).map_err(|error| {
            PreflightError::Failed(format!(
                "Cannot verify pi-mcp-adapter installation: \
                 `pi list` returned invalid UTF-8: {error}"
            ))
        })?;

        let packages = packages::inventory(stdout).map_err(|error| {
            PreflightError::Failed(format!(
                "Cannot verify pi-mcp-adapter installation: {error}. \
                 Run `{diagnostic_command}` to inspect the installation.\n{}",
                stdout.trim(),
            ))
        })?;
        for package in packages
            .iter()
            .filter(|package| packages::is_adapter(&package.source))
        {
            let Some(path) = &package.installed_path else {
                continue;
            };
            let manifest = path.join("package.json");
            let repair = recovery_command(context, "install npm:pi-mcp-adapter@3")?;
            let content = std::fs::read_to_string(&manifest).map_err(|error| {
                let message = format!("Cannot verify pi-mcp-adapter manifest {}: {error}. Repair with `{repair}` and retry ACE.", manifest.display());
                match error.kind() {
                    io::ErrorKind::NotFound => PreflightError::Unavailable(message),
                    _ => PreflightError::Failed(message),
                }
            })?;
            packages::validate_manifest(&content).map_err(|error| match error {
                packages::ManifestError::Invalid(error) => PreflightError::Failed(format!("Invalid pi-mcp-adapter manifest {}: {error}. Repair with `{repair}` and retry ACE.", manifest.display())),
                packages::ManifestError::Unsupported(version) => PreflightError::Unavailable(format!("pi-mcp-adapter {version} is unsupported; ACE requires adapter >=3.0.0 and <4.0.0. Run `{repair}` and retry ACE.")),
            })?;
        }
        Ok(packages)
    }
}

fn probe_error(error: io::Error, command: &str) -> PreflightError {
    if error.kind() == io::ErrorKind::NotFound {
        return PreflightError::Unavailable(format!(
            "Cannot start Pi command `{command}`: {error}. Install Pi from https://pi.dev \
             and ensure the configured executable is on PATH, then retry ACE."
        ));
    }
    PreflightError::Failed(format!(
        "Cannot verify pi-mcp-adapter installation: {error}. \
         Run `{command}` to diagnose, then retry ACE."
    ))
}

pub(super) struct LaunchSession<'a> {
    pub command: &'a [String],
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
}

impl LaunchSession<'_> {
    pub(super) fn run(&self, request: &SessionRequest) -> io::Result<()> {
        let prompt = match request.resume {
            ResumeMode::Fresh => Some(
                StorePrompt {
                    text: &request.session_prompt,
                }
                .run()?,
            ),
            ResumeMode::Latest => None,
        };
        let (program, prefix) = self
            .command
            .split_first()
            .map(|(program, prefix)| (program.as_str(), prefix))
            .unwrap_or(("pi", &[]));
        let mut args = prefix.to_vec();
        args.extend(build_session_args(
            self.model,
            self.effort,
            request,
            prompt.as_ref().map(|file| file.path()),
        )?);

        let status = SessionProcess::new(
            program.into(),
            args,
            request.env.clone(),
            request.project_dir.clone(),
        )
        .wait()?;
        drop(prompt);
        crate::platform::propagate_exit_status(status);
        Ok(())
    }
}

pub(super) struct RunOneShot<'a> {
    pub command: &'a [String],
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
}

impl RunOneShot<'_> {
    pub(super) fn run(&self, request: &OneShotRequest) -> io::Result<Output> {
        let (program, prefix) = self
            .command
            .split_first()
            .map(|(program, prefix)| (program.as_str(), prefix))
            .unwrap_or(("pi", &[]));
        let mut command = Command::new(program);
        command
            .args(prefix)
            .args(build_one_shot_args(self.model, self.effort, request))
            .envs(&request.env)
            .current_dir(&request.project_dir);
        let prompt = match &request.prompt {
            PromptInput::Inline(text) => Some(StorePrompt { text }.run()?),
            PromptInput::Stdin => None,
        };
        if let Some(prompt) = &prompt {
            command.stdin(prompt.reopen()?);
        } else {
            command.stdin(Stdio::inherit());
        }

        command.output()
    }
}
