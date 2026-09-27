mod common;

use common::TestEnv;
use predicates::prelude::*;

#[test]
fn branch_switch_rechecks_the_school_selected_backend_before_linking() {
    let env = TestEnv::new();
    let remote = env.setup_remote_school("test/pi-refresh");
    env.write_file("ace.toml", "school = \"test/pi-refresh\"\n");
    env.write_file(
        "data/ace/test/pi-refresh/school.toml",
        "name = \"main\"\nbackend = \"pi\"\n",
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
            "main uses Pi",
        ],
    );
    env.git_in(&remote.cache, &["checkout", "-b", "alternate"]);
    env.write_file(
        "data/ace/test/pi-refresh/school.toml",
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
            "alternate uses fixture backend",
        ],
    );
    env.write_file("data/ace/test/pi-refresh/.git/FETCH_HEAD", "fresh\n");
    env.write_executable("bin/pi", "#!/bin/sh\nprintf 'No packages installed.\n'\n");

    env.ace_with_path_prefix(&env.path("bin"))
        .args(["-p", "hello"])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("pi install npm:pi-mcp-adapter"));

    env.assert_not_exists(".pi");
    env.assert_not_exists(".claude");
    env.assert_not_exists(".gitignore");
}

#[test]
fn setup_links_the_effective_local_school_it_verified() {
    let env = TestEnv::new();
    env.setup_embedded_school("requested");
    env.write_file(
        "other/school.toml",
        "name = \"effective\"\nbackend = \"pi\"\n",
    );
    env.write_file(
        "other/skills/effective/SKILL.md",
        "# Effective school skill\n",
    );
    env.write_file("ace.local.toml", "school = \".:other\"\n");
    env.mkdir("adapter");
    env.write_file(
        "adapter/package.json",
        r#"{"name":"pi-mcp-adapter","version":"3.0.0"}"#,
    );
    env.write_executable("bin/pi", "#!/bin/sh\nprintf 'User packages:\\n  npm:pi-mcp-adapter@3.0.0\\n    %s/adapter\\n' \"$HOME\"\n");

    env.ace_with_path_prefix(&env.path("bin"))
        .args(["setup", "."])
        .assert()
        .success();

    env.assert_symlink(".pi/skills/effective", "other/skills/effective");
    env.assert_not_exists(".pi/skills/maverick");
    env.assert_contains("AGENTS.md", "effective");
}
