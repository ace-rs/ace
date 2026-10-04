mod common;

use common::TestEnv;
use predicates::prelude::*;

#[test]
fn branch_switch_verifies_the_final_school_backend_before_linking() {
    for project_config in [
        "school = \"test/backend-refresh\"\n",
        "school = \"test/backend-refresh\"\nbackend = \"checked\"\n",
    ] {
        let env = TestEnv::new();
        let remote = env.setup_remote_school("test/backend-refresh");
        env.write_file("ace.toml", project_config);
        env.write_file(
            "data/ace/test/backend-refresh/school.toml",
            "name = \"main\"\nbackend = \"checked\"\n[backends.checked]\nkind = \"flaude\"\ncmd = [\"main-wrapper\"]\nenv = { FLAUDE_PREFLIGHT = '{\"unavailable\":\"main prerequisite missing\"}' }\n",
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
                "main requires prerequisite",
            ],
        );
        env.git_in(&remote.cache, &["checkout", "-b", "alternate"]);
        env.write_file(
            "data/ace/test/backend-refresh/school.toml",
            "name = \"alternate\"\nbackend = \"flaude\"\n",
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
                "alternate is ready",
            ],
        );
        env.write_file("data/ace/test/backend-refresh/.git/FETCH_HEAD", "fresh\n");

        env.ace()
            .args(["-p", "hello"])
            .assert()
            .code(2)
            .stderr(predicate::str::contains("main prerequisite missing"));

        let calls: Vec<serde_json::Value> = env
            .read_file(".flaude-calls.jsonl")
            .lines()
            .map(|line| serde_json::from_str(line).expect("call record"))
            .collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["action"], "preflight");
        assert_eq!(calls[0]["cmd"], serde_json::json!(["main-wrapper"]));
        env.assert_not_exists(".claude");
        env.assert_not_exists(".gitignore");
        env.assert_not_exists(".flaude-exec-records.jsonl");
    }
}

#[test]
fn setup_links_the_effective_local_school_it_verified() {
    let env = TestEnv::new();
    env.setup_embedded_school("requested");
    env.write_file(
        "other/school.toml",
        "name = \"effective\"\nbackend = \"flaude\"\n",
    );
    env.write_file(
        "other/skills/effective/SKILL.md",
        "# Effective school skill\n",
    );
    env.write_file("ace.local.toml", "school = \".:other\"\n");

    env.ace().args(["setup", "."]).assert().success();

    env.assert_symlink(".claude/skills/effective", "other/skills/effective");
    env.assert_not_exists(".claude/skills/maverick");
    env.assert_contains("CLAUDE.md", "effective");
}
