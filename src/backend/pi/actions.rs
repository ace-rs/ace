use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use super::super::{Invocation, OneShotRequest, PreflightError, PromptInput, SessionRequest};
use super::{arguments, build_one_shot_args, build_session_args, packages, recovery_command};
use crate::session::{ResumeMode, SessionProcess, StorePrompt};

pub(super) struct ListPackages;

impl ListPackages {
    pub(super) fn run(
        &self,
        context: &Invocation,
    ) -> Result<Vec<packages::Package>, PreflightError> {
        let diagnostic_command = recovery_command(context, "list --no-approve")?;
        let output = inventory_command(context)?
            .output()
            .map_err(|error| probe_error(error, &diagnostic_command))?;
        let packages = inventory_output(&output, &diagnostic_command)?;
        for package in packages
            .iter()
            .filter(|package| packages::is_adapter(&package.source))
        {
            let Some(path) = &package.installed_path else {
                continue;
            };
            let manifest = path.join("package.json");
            let repair = recovery_command(context, "install npm:pi-mcp-adapter@3")?;
            verify_manifest(std::fs::read_to_string(&manifest), &manifest, &repair)?;
        }
        Ok(packages)
    }
}

fn verify_manifest(
    content: io::Result<String>,
    manifest: &Path,
    repair: &str,
) -> Result<(), PreflightError> {
    let content = content.map_err(|error| {
        let message = format!(
            "Cannot verify pi-mcp-adapter manifest {}: {error}. \
             Repair with `{repair}` and retry ACE.",
            manifest.display(),
        );
        match error.kind() {
            io::ErrorKind::NotFound => PreflightError::Unavailable(message),
            _ => PreflightError::Failed(message),
        }
    })?;

    packages::validate_manifest(&content).map_err(|error| match error {
        packages::ManifestError::Invalid(error) => PreflightError::Failed(format!(
            "Invalid pi-mcp-adapter manifest {}: {error}. Repair with `{repair}` and retry ACE.",
            manifest.display(),
        )),
        packages::ManifestError::Unsupported(version) => PreflightError::Unavailable(format!(
            "pi-mcp-adapter {version} is unsupported; ACE requires adapter >=3.0.0 and <4.0.0. \
             Run `{repair}` and retry ACE."
        )),
    })
}

fn inventory_command(context: &Invocation) -> Result<Command, PreflightError> {
    let mut probe = context.clone();
    probe.command = arguments::probe_command(context)?;
    let mut command = probe.process();
    command
        .args(["list", "--no-approve"])
        .env("NO_COLOR", "1")
        .env("PI_OFFLINE", "1")
        .stdin(Stdio::null());
    Ok(command)
}

fn inventory_output(
    output: &Output,
    diagnostic_command: &str,
) -> Result<Vec<packages::Package>, PreflightError> {
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

    packages::inventory(stdout).map_err(|error| {
        PreflightError::Failed(format!(
            "Cannot verify pi-mcp-adapter installation: {error}. \
             Run `{diagnostic_command}` to inspect the installation.\n{}",
            stdout.trim(),
        ))
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::Kind;
    use std::collections::HashMap;
    use std::ffi::OsStr;
    use std::path::Path;

    #[test]
    fn manifest_supported_installation_passes_verification() {
        verify_manifest(
            Ok(r#"{"name":"pi-mcp-adapter","version":"3.0.0"}"#.into()),
            Path::new("/agent/adapter/package.json"),
            "pi install npm:pi-mcp-adapter@3",
        )
        .expect("supported installed adapter");
    }

    #[test]
    fn manifest_absence_is_unavailable_with_instance_repair_guidance() {
        let manifest = Path::new("/selected-agent/adapter/package.json");
        let repair =
            "PI_CODING_AGENT_DIR=/selected-agent wrapper pi-instance install npm:pi-mcp-adapter@3";
        let error = verify_manifest(
            Err(io::Error::new(io::ErrorKind::NotFound, "manifest missing")),
            manifest,
            repair,
        )
        .expect_err("missing manifest");

        assert!(matches!(error, PreflightError::Unavailable(_)));
        assert!(error.to_string().contains(&manifest.display().to_string()));
        assert!(error.to_string().contains(repair));
        assert!(error.to_string().contains("manifest missing"));
    }

    #[test]
    fn manifest_unsupported_version_explains_supported_range_and_repair() {
        let repair = "wrapper pi-instance install npm:pi-mcp-adapter@3";
        let error = verify_manifest(
            Ok(r#"{"name":"pi-mcp-adapter","version":"2.9.0"}"#.into()),
            Path::new("/agent/adapter/package.json"),
            repair,
        )
        .expect_err("unsupported installed adapter");

        assert!(matches!(error, PreflightError::Unavailable(_)));
        assert!(error.to_string().contains("2.9.0"));
        assert!(error.to_string().contains(">=3.0.0 and <4.0.0"));
        assert!(error.to_string().contains(repair));
    }

    #[test]
    fn manifest_malformed_content_fails_verification_with_source_and_repair() {
        let manifest = Path::new("/agent/adapter/package.json");
        let repair = "pi install npm:pi-mcp-adapter@3";
        let error = verify_manifest(Ok("not json".into()), manifest, repair)
            .expect_err("malformed installed manifest");

        assert!(matches!(error, PreflightError::Failed(_)));
        assert!(
            error
                .to_string()
                .contains("Invalid pi-mcp-adapter manifest")
        );
        assert!(error.to_string().contains(&manifest.display().to_string()));
        assert!(error.to_string().contains(repair));
    }

    #[test]
    fn manifest_read_denial_is_failed_verification_with_source_and_repair() {
        let manifest = Path::new("/agent/adapter/package.json");
        let repair = "pi install npm:pi-mcp-adapter@3";
        let error = verify_manifest(
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "permission denied",
            )),
            manifest,
            repair,
        )
        .expect_err("unreadable installed manifest");

        assert!(matches!(error, PreflightError::Failed(_)));
        assert!(error.to_string().contains(&manifest.display().to_string()));
        assert!(error.to_string().contains(repair));
        assert!(error.to_string().contains("permission denied"));
    }

    // The user-only, offline package probe is specified in docs/spec/backends/pi.md.
    #[test]
    fn package_probe_preserves_instance_and_excludes_launch_options() {
        let context = Invocation {
            kind: Kind::Pi,
            command: vec![
                "wrapper".into(),
                "pi-instance".into(),
                "--mcp-config=other.json".into(),
            ],
            project_dir: "/project".into(),
            env: HashMap::from([
                ("HOME".into(), "/selected-home".into()),
                ("PI_CODING_AGENT_DIR".into(), "/selected-agent".into()),
                ("PI_OFFLINE".into(), "0".into()),
                ("NO_COLOR".into(), "0".into()),
            ]),
            model: Some("provider/model".into()),
            effort: Some("high".into()),
            extra_args: vec!["--mode".into(), "rpc".into()],
        };

        let command = inventory_command(&context).expect("valid wrapper command");
        let environment = command.get_envs().collect::<HashMap<_, _>>();

        assert_eq!(command.get_program(), OsStr::new("wrapper"));
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["pi-instance", "list", "--no-approve"]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("/project")));
        assert_eq!(
            environment[OsStr::new("HOME")],
            Some(OsStr::new("/selected-home"))
        );
        assert_eq!(
            environment[OsStr::new("PI_CODING_AGENT_DIR")],
            Some(OsStr::new("/selected-agent"))
        );
        assert_eq!(environment[OsStr::new("PI_OFFLINE")], Some(OsStr::new("1")));
        assert_eq!(environment[OsStr::new("NO_COLOR")], Some(OsStr::new("1")));
    }

    #[cfg(unix)]
    fn output(code: i32, stdout: &[u8], stderr: &[u8]) -> Output {
        use std::os::unix::process::ExitStatusExt;
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_failed_or_diagnostic_package_probes_even_with_valid_inventory() {
        for (code, stderr) in [(7, ""), (0, "extension failed")] {
            let output = output(code, b"No packages installed.\n", stderr.as_bytes());
            let result = inventory_output(&output, "wrapper list --no-approve");
            let Err(PreflightError::Failed(message)) = result else {
                panic!("probe diagnostics must fail verification");
            };
            assert!(message.contains(stderr), "{message}");
            assert!(message.contains(&output.status.to_string()), "{message}");
            assert!(message.contains("wrapper list --no-approve"), "{message}");
            assert!(message.contains("No packages installed."), "{message}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_non_utf8_inventory_before_package_parsing() {
        let result = inventory_output(
            &output(0, b"User packages:\n  npm:other\xff\n", b""),
            "pi list --no-approve",
        );
        let Err(PreflightError::Failed(message)) = result else {
            panic!("invalid UTF-8 is not a package declaration");
        };
        assert!(message.contains("invalid UTF-8"), "{message}");
    }

    #[cfg(unix)]
    #[test]
    fn accepts_clean_inventory_but_explains_unrecognized_output() {
        let packages = inventory_output(
            &output(
                0,
                b"User packages:\n  npm:other\n    /agent/other\n",
                b" \n",
            ),
            "wrapper list --no-approve",
        )
        .expect("clean inventory");
        assert_eq!(packages.len(), 1);
        assert_eq!(
            packages[0].installed_path.as_deref(),
            Some(Path::new("/agent/other"))
        );

        let error = inventory_output(
            &output(0, b"new upstream format", b""),
            "wrapper list --no-approve",
        )
        .expect_err("unknown output format");
        assert!(matches!(error, PreflightError::Failed(_)));
        assert!(error.to_string().contains("wrapper list --no-approve"));
        assert!(error.to_string().contains("new upstream format"));
    }

    #[test]
    fn distinguishes_missing_executable_from_other_probe_io_failures() {
        let missing = probe_error(
            io::Error::new(io::ErrorKind::NotFound, "missing executable"),
            "wrapper pi-instance list --no-approve",
        );
        assert!(matches!(missing, PreflightError::Unavailable(_)));
        assert!(missing.to_string().contains("https://pi.dev"));
        assert!(missing.to_string().contains("wrapper pi-instance"));

        let denied = probe_error(
            io::Error::new(io::ErrorKind::PermissionDenied, "permission denied"),
            "wrapper pi-instance list --no-approve",
        );
        assert!(matches!(denied, PreflightError::Failed(_)));
        assert!(denied.to_string().contains("permission denied"));
    }
}
