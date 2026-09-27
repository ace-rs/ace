mod common;

use common::TestEnv;
use predicates::prelude::*;

fn missing_adapter(env: &TestEnv) {
    env.write_executable("bin/pi", "#!/bin/sh\nprintf 'No packages installed.\n'\n");
}

#[test]
fn setup_checks_school_selected_pi_before_publishing_project() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    env.write_file("school.toml", "name = \"pi-school\"\nbackend = \"pi\"\n");
    missing_adapter(&env);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["setup", "."])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("pi install npm:pi-mcp-adapter"));
    env.assert_not_exists("ace.toml");
    env.assert_not_exists(".pi");
    env.assert_not_exists(".gitignore");
}

#[test]
fn missing_adapter_does_not_clear_mcp_exclusion() {
    let env = TestEnv::new();
    env.write_dogfood_school(
        "name = \"pi-school\"\nbackend = \"pi\"\n[[mcp]]\nname = \"sample\"\nurl = \"https://example.com/mcp\"\n",
    );
    env.write_file("ace.local.toml", "exclude_mcp = [\"sample\"]\n");
    missing_adapter(&env);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["mcp", "register", "sample"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("pi install npm:pi-mcp-adapter"));
    assert_eq!(
        env.read_file("ace.local.toml"),
        "exclude_mcp = [\"sample\"]\n"
    );
}

#[test]
fn prepare_checks_pi_before_linking_or_writing_gitignore() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    env.write_file("ace.toml", "school = \".\"\nbackend = \"pi\"\n");
    missing_adapter(&env);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["-p", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("pi install npm:pi-mcp-adapter"));
    env.assert_not_exists(".pi");
    env.assert_not_exists(".gitignore");
    assert_eq!(
        env.read_file("ace.toml"),
        "school = \".\"\nbackend = \"pi\"\n"
    );
}

#[test]
fn pi_selection_allows_configuration_inspection_without_installation() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "backend = \"pi\"\n");
    env.ace()
        .args(["--pi", "config", "get", "backend"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pi"));
}

fn recording_pi(env: &TestEnv) {
    env.mkdir("adapter");
    env.write_file(
        "adapter/package.json",
        "{\"name\":\"pi-mcp-adapter\",\"version\":\"3.0.0\"}",
    );
    env.write_executable(
        "bin/pi-wrapper",
        r#"#!/bin/sh
test "$1" = prefix || exit 91
shift
printf '%s\n' "$PI_CODING_AGENT_DIR:$CONTEXT:$PWD:$*" >> "$HOME/pi-record"
if test "$1" = list; then
    test "$2" = --no-approve || exit 92
    test "$PI_OFFLINE" = 1 || exit 93
    printf 'User packages:\n  npm:pi-mcp-adapter@3.0.0\n    %s/adapter\n' "$HOME"
    exit 0
fi
if test "$1" = --print; then
    cat > "$HOME/pi-prompt"
fi
printf 'pi output\n'
if test "$PI_TEST_SIGNAL" = yes; then kill -TERM $$; fi
exit "${PI_TEST_EXIT:-0}"
"#,
    );
    env.write_file(
        "ace.toml",
        r#"school = "."
backend = "custom-pi"
env = { CONTEXT = "global", PI_CODING_AGENT_DIR = "global-agent" }
[backends.custom-pi]
kind = "pi"
cmd = ["{{ project_dir }}/bin/pi-wrapper", "prefix"]
env = { CONTEXT = "backend", PI_CODING_AGENT_DIR = "{{ project_dir }}/chosen-agent" }
model = "test-model"
effort = "low"
"#,
    );
}

#[test]
fn named_pi_preserves_context_and_literal_one_shot_prompt() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    recording_pi(&env);
    env.write_file("secret-file", "do not attach this file");
    env.ace()
        .args(["-p", "@secret-file\n--not-an-option"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pi output"));
    assert_eq!(env.read_file("pi-prompt"), "@secret-file\n--not-an-option");
    let record = env.read_file("pi-record");
    assert!(
        record.contains(&format!(
            "{}:backend:{}:list",
            env.path("chosen-agent").display(),
            env.root().display()
        )),
        "{record}"
    );
    assert!(record.contains("--model test-model"), "{record}");
    assert!(record.contains("--thinking low"), "{record}");
    env.assert_symlink(".pi/skills/maverick", "skills/maverick");
}

#[test]
fn pi_propagates_nonzero_child_exit() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    recording_pi(&env);
    env.ace()
        .env("PI_TEST_EXIT", "7")
        .args(["-p", "hello"])
        .assert()
        .code(7);
}

#[test]
fn pi_propagates_child_signal_exit() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    recording_pi(&env);
    env.ace()
        .env("PI_TEST_SIGNAL", "yes")
        .args(["-p", "hello"])
        .assert()
        .code(143);
}

#[test]
fn failed_package_probe_is_not_reported_as_missing_adapter() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    env.write_file("ace.toml", "school = \".\"\nbackend = \"pi\"\n");
    env.write_executable(
        "bin/pi",
        "#!/bin/sh\nprintf 'broken settings\\n' >&2\nexit 4\n",
    );
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["-p", "hello"])
        .assert()
        .code(3)
        .stderr(
            predicate::str::contains("Cannot verify")
                .and(predicate::str::contains("broken settings")),
        );
    env.assert_not_exists(".pi");
}

#[test]
fn missing_pi_names_executable_and_repair() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    env.write_file("ace.toml", "school = \".\"\nbackend = \"pi\"\n[backends.pi]\ncmd = [\"{{ project_dir }}/missing-pi\"]\n");
    env.ace()
        .args(["-p", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("missing-pi").and(predicate::str::contains("Install Pi")));
    env.assert_not_exists(".pi");
}

#[test]
fn mcp_inventory_reports_missing_pi_as_an_unavailable_prerequisite() {
    let env = TestEnv::new();
    env.write_file(
        "ace.toml",
        "backend = \"pi\"\n[backends.pi]\ncmd = [\"{{ project_dir }}/missing-pi\"]\n",
    );
    env.ace()
        .arg("mcp")
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Install Pi"));
}

#[test]
fn disabled_extensions_fail_before_probe_or_project_changes() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    recording_pi(&env);
    env.ace()
        .args(["-p", "hello", "--", "--no-extensions"])
        .assert()
        .code(1)
        .stderr(predicate::str::contains("--no-extensions"));
    env.assert_not_exists("pi-record");
    env.assert_not_exists(".pi");
}

#[test]
fn pi_emits_nested_skills_and_preserves_shared_agent_skills() {
    let env = TestEnv::new();
    env.setup_embedded_school("pi-school");
    env.write_file("skills/rust/review/SKILL.md", "# Rust review\n");
    env.write_file(".agents/skills/personal/SKILL.md", "# Personal skill\n");
    recording_pi(&env);
    env.ace().args(["-p", "hello"]).assert().success();
    env.assert_symlink(".pi/skills/rust/review", "skills/rust/review");
    assert_eq!(
        env.read_file(".agents/skills/personal/SKILL.md"),
        "# Personal skill\n"
    );
}

#[test]
fn backend_binding_waits_for_the_final_school_branch() {
    let env = TestEnv::new();
    let remote = env.setup_remote_school("test/pi-binding");
    env.write_file(
        "ace.toml",
        "school = \"test/pi-binding\"\nbackend = \"school-pi\"\n",
    );
    env.write_file(
        "data/ace/test/pi-binding/school.toml",
        "name = \"main\"\n[backends.school-pi]\nkind = \"pi\"\n",
    );
    env.git_in(&remote.cache, &["add", "school.toml"]);
    env.git_in(
        &remote.cache,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "define Pi instance",
        ],
    );
    env.git_in(&remote.cache, &["checkout", "-b", "alternate"]);
    env.write_file(
        "data/ace/test/pi-binding/school.toml",
        "name = \"alternate\"\n",
    );
    env.git_in(&remote.cache, &["add", "school.toml"]);
    env.git_in(
        &remote.cache,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "no custom instance",
        ],
    );
    env.write_file("data/ace/test/pi-binding/.git/FETCH_HEAD", "fresh\n");
    missing_adapter(&env);
    env.ace_with_path_prefix(&env.path("bin"))
        .args(["-p", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("pi install npm:pi-mcp-adapter"));
    env.assert_not_exists(".pi");
}
