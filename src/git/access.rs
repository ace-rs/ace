//! Repository access and terminal handoff for Git transport operations.

use std::fs::File;
use std::io::{self, Read, Seek};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

use super::GitError;

pub enum Interaction {
    Attended,
    Unattended,
}

pub struct Probe<'a> {
    pub url: &'a str,
}

impl Probe<'_> {
    pub fn run(&self, cwd: &Path) -> Result<(), GitError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut command = unattended_command(cwd, deadline)?;
        command
            .args(["ls-remote", "--", self.url])
            .stdout(Stdio::null());

        run_bounded(
            &mut command,
            deadline.saturating_duration_since(Instant::now()),
        )
    }
}

pub struct Clone<'a> {
    pub url: &'a str,
    pub dest: &'a Path,
    pub interaction: Interaction,
}

impl Clone<'_> {
    pub fn run(&self, cwd: &Path) -> Result<(), GitError> {
        let mut command = match self.interaction {
            Interaction::Attended => Command::new("git"),
            Interaction::Unattended => {
                unattended_command(cwd, Instant::now() + Duration::from_secs(5))?
            }
        };
        forward_credential_config(&mut command, cwd)?;
        command
            .current_dir(cwd)
            .args(["clone", "--no-tags", "--", self.url])
            .arg(self.dest);
        let cmd = format!("clone --no-tags {}", self.url);

        match self.interaction {
            Interaction::Attended => {
                let _supervision = crate::platform::begin_child_supervision();
                let status = command
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit())
                    .status()
                    .map_err(|source| GitError::Exec {
                        cmd: cmd.clone(),
                        source,
                    })?;
                check_status(status, cmd, String::new())
            }
            Interaction::Unattended => {
                let output =
                    command
                        .stdout(Stdio::null())
                        .output()
                        .map_err(|source| GitError::Exec {
                            cmd: cmd.clone(),
                            source,
                        })?;
                check_status(
                    output.status,
                    cmd,
                    String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                )
            }
        }
    }
}

fn forward_credential_config(command: &mut Command, cwd: &Path) -> Result<(), GitError> {
    let config_error = |source| GitError::Exec {
        cmd: "config: read credential settings".to_owned(),
        source,
    };
    let mut settings_file = tempfile::tempfile().map_err(config_error)?;
    let mut config = Command::new("git");
    config
        .current_dir(cwd)
        .args(["config", "--null", "--get-regexp", "^credential\\."])
        .stdout(settings_file.try_clone().map_err(config_error)?);
    match run_bounded(&mut config, Duration::from_secs(5)) {
        Ok(()) => {}
        Err(GitError::Exit { status, .. }) if status.code() == Some(1) => return Ok(()),
        Err(error) => return Err(error),
    }
    settings_file.rewind().map_err(config_error)?;
    let mut settings = String::new();
    settings_file
        .read_to_string(&mut settings)
        .map_err(config_error)?;

    // Reset the inherited helper chain, then let Git apply the original ordered URL scopes.
    command.env("ACE_GIT_CREDENTIAL_RESET", "");
    command.arg("--config-env=credential.helper=ACE_GIT_CREDENTIAL_RESET");
    for (index, setting) in settings.split_terminator('\0').enumerate() {
        match setting.split_once('\n') {
            Some((key, value)) => {
                let variable = format!("ACE_GIT_CREDENTIAL_{index}");
                command.arg(format!("--config-env={key}={variable}"));
                command.env(variable, value);
            }
            None => {
                // Preserve valueless syntax; Git validates each setting's accepted values.
                command.args(["-c", setting]);
            }
        }
    }
    Ok(())
}

fn run_bounded(command: &mut Command, timeout: Duration) -> Result<(), GitError> {
    let cmd = command
        .get_args()
        .map(|arg| arg.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let mut execute = || -> io::Result<(ExitStatus, String)> {
        let mut diagnostics = tempfile::tempfile()?;
        command
            .stdin(Stdio::null())
            .stderr(diagnostics.try_clone()?);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            command.process_group(0);
        }

        let mut child = command.spawn()?;
        let status = wait_bounded(&mut child, timeout);
        let stderr = read_diagnostics(&mut diagnostics)?;
        let status =
            status.map_err(|error| io::Error::new(error.kind(), format!("{error}\n{stderr}")))?;
        Ok((status, stderr))
    };
    let (status, stderr) = execute().map_err(|source| GitError::Exec {
        cmd: cmd.clone(),
        source,
    })?;

    check_status(status, cmd, stderr)
}

pub(super) fn unattended_command(cwd: &Path, deadline: Instant) -> Result<Command, GitError> {
    let mut command = super::git_command();
    command.current_dir(cwd);
    if std::env::var_os("GIT_SSH_COMMAND").is_some() {
        return Ok(command);
    }

    let config_error = |source| GitError::Exec {
        cmd: "config --null --get core.sshCommand".to_owned(),
        source,
    };
    let mut configured_ssh = tempfile::tempfile().map_err(config_error)?;
    let mut config = Command::new("git");
    config
        .current_dir(cwd)
        .args(["config", "--null", "--get", "core.sshCommand"])
        .stdout(configured_ssh.try_clone().map_err(config_error)?);
    match run_bounded(
        &mut config,
        deadline.saturating_duration_since(Instant::now()),
    ) {
        Ok(()) => {
            configured_ssh.rewind().map_err(config_error)?;
            let mut value = String::new();
            configured_ssh
                .read_to_string(&mut value)
                .map_err(config_error)?;
            let value = value.strip_suffix('\0').ok_or_else(|| {
                config_error(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid Git SSH configuration output",
                ))
            })?;
            // Git clone does not load the source directory's repository-local config.
            command.env("GIT_SSH_COMMAND", value);
        }
        Err(GitError::Exit { status, .. }) if status.code() == Some(1) => {
            if std::env::var_os("GIT_SSH").is_none() {
                command.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
            }
        }
        Err(error) => return Err(error),
    }

    Ok(command)
}

fn wait_bounded(child: &mut Child, timeout: Duration) -> io::Result<ExitStatus> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(error) => {
                terminate(child)?;
                return Err(error);
            }
        }
        if Instant::now() >= deadline {
            terminate(child)?;
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "repository access check timed out",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn terminate(child: &mut Child) -> io::Result<()> {
    #[cfg(unix)]
    let output = Command::new("/bin/kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .output()?;
    #[cfg(windows)]
    let output = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &child.id().to_string()])
        .output()?;

    if !output.status.success() && child.try_wait()?.is_none() {
        child.kill()?;
        child.wait()?;
        return Err(io::Error::other(format!(
            "terminate Git process group: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    child.wait()?;
    Ok(())
}

fn read_diagnostics(file: &mut File) -> io::Result<String> {
    file.rewind()?;
    let mut bytes = Vec::new();
    file.take(65_536).read_to_end(&mut bytes)?;
    Ok(String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn check_status(status: ExitStatus, cmd: String, stderr: String) -> Result<(), GitError> {
    if status.success() {
        return Ok(());
    }
    Err(GitError::Exit {
        cmd,
        status,
        stderr,
    })
}
