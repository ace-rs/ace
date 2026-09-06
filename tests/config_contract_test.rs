mod common;

use common::TestEnv;
use predicates::prelude::*;

#[test]
fn explicit_local_default_overrides_inherited_yolo() {
    let env = TestEnv::new();
    env.write_file("config/ace/ace.toml", "trust = \"yolo\"\n");

    env.ace()
        .args(["config", "set", "trust", "default"])
        .assert()
        .success();

    env.assert_contains("ace.local.toml", "trust = \"default\"");
    env.ace()
        .args(["config", "get", "trust"])
        .assert()
        .success()
        .stdout("default\n");
    env.ace()
        .args(["config", "explain", "trust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("trust = \"default\"  [local]"));
}

#[test]
fn explicit_cli_default_overrides_local_yolo() {
    let env = TestEnv::new();
    env.write_file("ace.local.toml", "trust = \"yolo\"\n");

    env.ace()
        .args(["--trust", "default", "config", "get", "trust"])
        .assert()
        .success()
        .stdout("default\n");
    env.ace()
        .args(["--trust", "default", "config", "explain", "trust"])
        .assert()
        .success()
        .stdout(predicate::str::contains("trust = \"default\"  [override]"));
    env.assert_contains("ace.local.toml", "trust = \"yolo\"");
}

#[test]
fn explicit_trust_default_takes_precedence_over_legacy_yolo_in_same_layer() {
    let env = TestEnv::new();
    env.write_file("ace.local.toml", "trust = \"default\"\nyolo = true\n");

    env.ace()
        .args(["config", "get", "trust"])
        .assert()
        .success()
        .stdout("default\n");
}

#[test]
fn setting_one_field_preserves_unknown_content_and_comments() {
    let env = TestEnv::new();
    env.write_file(
        "ace.toml",
        "# shared settings\nbackend = \"claude\" # chosen backend\neffort = \"high\"\n\
         [future]\nmode = \"custom\" # retain this\n",
    );

    env.ace()
        .args(["config", "set", "backend", "codex"])
        .assert()
        .success();

    env.assert_contains("ace.toml", "# shared settings");
    env.assert_contains("ace.toml", "# chosen backend");
    env.assert_contains("ace.toml", "effort = \"high\"");
    env.assert_contains("ace.toml", "mode = \"custom\" # retain this");
    env.ace()
        .args(["config", "get", "backend"])
        .assert()
        .success()
        .stdout("codex\n");
}

#[cfg(unix)]
#[test]
fn config_set_follows_a_relative_symlink_to_a_missing_target() {
    let env = TestEnv::new();
    std::os::unix::fs::symlink("personal/settings.toml", env.path("ace.local.toml"))
        .expect("create relative config link");

    env.ace()
        .args(["config", "set", "trust", "default"])
        .assert()
        .success();

    assert_eq!(
        std::fs::read_link(env.path("ace.local.toml")).expect("retained link"),
        std::path::Path::new("personal/settings.toml")
    );
    env.assert_contains("personal/settings.toml", "trust = \"default\"");
}

#[test]
fn config_set_preserves_quoted_key_formatting_in_regular_dotted_and_inline_tables() {
    let documents = [
        "[env]\n# keep key explanation\n'TEAM.KEY'  =  \"old\" # keep trailing comment\n",
        "# keep dotted explanation\nenv . 'TEAM.KEY'  =  \"old\" # keep trailing comment\n",
        "env = {\n# keep inline explanation\n'TEAM.KEY'  =  \"old\", # keep trailing comment\n}\n",
    ];
    for original in documents {
        let env = TestEnv::new();
        env.write_file("ace.toml", original);

        env.ace()
            .args(["config", "set", "env.TEAM.KEY", "new"])
            .assert()
            .success();

        assert_eq!(
            env.read_file("ace.toml"),
            original.replace("\"old\"", "\"new\"")
        );
    }
}
