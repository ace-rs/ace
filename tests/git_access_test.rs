#![cfg(unix)]

mod common;

use common::TestEnv;
use std::process::Command;
use std::time::{Duration, Instant};

fn denied_transport() -> TestEnv {
    let env = TestEnv::new();
    env.git_init();
    env.write_executable(
        "bin/git",
        r#"#!/bin/sh
case "$*" in
  *'config '*'--get core.sshCommand')
    printf 'ssh -i /configured/identity'
    if [ "${CONFIG_SSH_TRAILING_NEWLINE:-0}" = 1 ]; then printf '\n'; fi
    printf '\000'
    exit "${CONFIG_SSH_STATUS:-1}"
    ;;
  *ls-remote*)
    printf 'ssh=%s askpass=%s interactive=%s\n' "${GIT_SSH_COMMAND-unset}" "${GIT_ASKPASS-unset}" "${GCM_INTERACTIVE-unset}" >> "$HOME/transport-env"
    printf 'ssh-askpass=%s ssh-input=%s\n' "${SSH_ASKPASS-unset}" "${SSH_ASKPASS_REQUIRE-unset}" >> "$HOME/transport-env"
    case "$*" in
      *git@github.com*)
        printf 'SSH repository access denied\n' >&2
        if [ "${HANG_SSH:-0}" = 1 ]; then
          /bin/sleep 30 &
          printf '%s\n' "$!" > "$HOME/transport-child"
          wait
        fi
        ;;
      *) printf 'HTTPS repository access denied\n' >&2 ;;
    esac
    exit 128
    ;;
  *) exit 1 ;;
esac
"#,
    );
    env
}

#[test]
fn failed_probes_preserve_both_transport_diagnostics_and_custom_ssh() {
    let env = denied_transport();
    env.ace_with_path_prefix(&env.path("bin"))
        .env("GIT_SSH_COMMAND", "ssh -i /configured/identity")
        .env("GIT_ASKPASS", "/must/not/launch")
        .env("GCM_INTERACTIVE", "1")
        .args(["--yes", "setup", "example/school"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("SSH repository access denied"))
        .stderr(predicates::str::contains("HTTPS repository access denied"));

    env.assert_contains(
        "transport-env",
        "ssh=ssh -i /configured/identity askpass= interactive=0",
    );
    env.assert_not_exists("ace.toml");
    env.assert_contains("transport-env", "ssh-askpass= ssh-input=force");
}

#[test]
fn configured_ssh_command_is_not_replaced_by_default_identity_settings() {
    let env = denied_transport();
    env.ace_with_path_prefix(&env.path("bin"))
        .env("CONFIG_SSH_STATUS", "0")
        .env("CONFIG_SSH_TRAILING_NEWLINE", "1")
        .args(["--yes", "setup", "example/school"])
        .assert()
        .failure();

    env.assert_contains(
        "transport-env",
        "ssh=ssh -i /configured/identity\n askpass= interactive=0",
    );
}

#[test]
fn stalled_probe_times_out_and_terminates_its_transport_child() {
    let env = denied_transport();
    let started = Instant::now();
    env.ace_with_path_prefix(&env.path("bin"))
        .env("HANG_SSH", "1")
        .args(["--yes", "setup", "example/school"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("timed out"))
        .stderr(predicates::str::contains("SSH repository access denied"));

    assert!(started.elapsed() < Duration::from_secs(6));
    let pid = env.read_file("transport-child");
    let alive = Command::new("/bin/kill")
        .env_clear()
        .args(["-0", pid.trim()])
        .output()
        .expect("check transport child termination");
    assert!(
        !alive.status.success(),
        "timed-out SSH child must not survive"
    );
    env.assert_not_exists("ace.toml");
}

fn credential_transport(use_http_path: &str) -> (TestEnv, assert_cmd::Command) {
    let env = TestEnv::new();
    env.git_init();
    env.write_file(
        ".gitconfig",
        "[credential]\nhelper = !$HOME/bin/ignored-helper\n",
    );
    let config = env.read_file(".git/config");
    env.write_file(
        ".git/config",
        &format!(
            "{config}\n[credential]\nprovider = github\n{use_http_path}\n[credential \"https://github.com\"]\nhelper =\nhelper = !$HOME/bin/username-helper\nhelper = !$HOME/bin/password-helper\n"
        ),
    );
    env.write_executable(
        "bin/ignored-helper",
        "#!/bin/sh\nprintf 'ignored\\n' >> \"$HOME/helper-calls\"\nexit 1\n",
    );
    env.write_executable(
        "bin/username-helper",
        "#!/bin/sh\nprintf 'username\\n' >> \"$HOME/helper-calls\"\nprintf 'username=fixture\\n'\n",
    );
    env.write_executable(
        "bin/password-helper",
        "#!/bin/sh\nprintf 'password\\n' >> \"$HOME/helper-calls\"\n",
    );
    env.write_executable(
        "bin/command-helper",
        "#!/bin/sh\nprintf 'command\\n' >> \"$HOME/helper-calls\"\n",
    );
    env.write_executable(
        "bin/git",
        r#"#!/bin/sh
case "$*" in
  *ls-remote*git@github.com*) exit 1 ;;
  *ls-remote*https://github.com*) exit 0 ;;
  *) exec "$REAL_GIT" "$@" ;;
esac
"#,
    );
    env.write_executable(
        "transport/git-remote-https",
        r#"#!/bin/sh
provider=$("$REAL_GIT" config --get credential.provider)
use_http_path=$("$REAL_GIT" config --bool --get credential.useHttpPath)
if [ "$provider" != github ] || [ "$use_http_path" != true ]; then
  printf 'missing-provider-or-path-setting\n' >> "$HOME/helper-calls"
  exit 1
fi
printf 'protocol=https\nhost=github.com\n\n' | "$REAL_GIT" credential fill > /dev/null
exit 1
"#,
    );
    let original_path = std::env::var_os("PATH").expect("test runner PATH");
    let git_path = std::env::split_paths(&original_path)
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("Git executable for isolated transport fixture");

    let mut command = env.ace_with_path_prefix(&env.path("bin"));
    command
        .env("REAL_GIT", git_path)
        .env("GIT_EXEC_PATH", env.path("transport"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_PARAMETERS",
            "'credential.helper=!$HOME/bin/command-helper'",
        )
        .args(["--yes", "setup", "example/school"]);

    (env, command)
}

#[test]
fn clone_reuses_project_credential_helpers_in_their_configured_order() {
    let (env, mut command) = credential_transport("useHttpPath = true");
    command.assert().failure();

    assert_eq!(
        env.read_file("helper-calls"),
        "username\npassword\ncommand\n"
    );
    env.assert_not_exists("ace.toml");
}

#[test]
fn clone_preserves_valueless_settings_for_git_to_validate() {
    let (env, mut command) = credential_transport("useHttpPath");
    command.assert().failure().stderr(predicates::str::contains(
        "missing value for 'credential.usehttppath'",
    ));

    env.assert_not_exists("ace.toml");
}
