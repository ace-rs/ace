mod common;

use common::TestEnv;

fn setup_school_with_skills(env: &TestEnv, name: &str, skills: &[&str]) {
    env.git_init();
    env.write_file("school.toml", &format!("name = \"{name}\"\n"));
    for skill in skills {
        env.mkdir(&format!("skills/{skill}"));
        env.write_file(&format!("skills/{skill}/SKILL.md"), "# stub\n");
    }
    env.ace().args(["setup", "."]).assert().success();
}

#[test]
fn skills_lists_all_when_no_filter() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts1", &["alpha", "beta", "gamma"]);

    let output = env.ace().args(["skills"]).output().expect("ace skills");
    assert!(output.status.success(), "ace skills should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("NAME\tTIER\tSTATUS\tREASON"), "header missing");
    assert!(stdout.contains("alpha"));
    assert!(stdout.contains("beta"));
    assert!(stdout.contains("gamma"));
    assert!(stdout.contains("active"));
}

#[test]
fn skills_names_only_prints_bare_names() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts2", &["alpha", "beta"]);

    let output = env.ace().args(["skills", "--names"]).output().expect("ace skills --names");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, vec!["alpha", "beta"]);
}

#[test]
fn skills_default_hides_excluded() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts3", &["alpha", "beta"]);

    env.ace().args(["skills", "exclude", "alpha"]).assert().success();

    let output = env.ace().args(["skills", "--names"]).output().expect("ace skills");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, vec!["beta"], "alpha should be hidden");
}

#[test]
fn skills_all_flag_shows_excluded() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts4", &["alpha", "beta"]);

    env.ace().args(["skills", "exclude", "alpha"]).assert().success();

    let output = env.ace().args(["skills", "--all", "--names"]).output().expect("ace skills --all");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, vec!["alpha", "beta"]);
}

#[test]
fn skills_include_writes_to_project_ace_toml() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts5", &["alpha"]);

    env.ace().args(["skills", "include", "alpha"]).assert().success();

    let toml = std::fs::read_to_string(env.path("ace.toml")).expect("read ace.toml");
    assert!(toml.contains("include_skills"), "missing include_skills key:\n{toml}");
    assert!(toml.contains("alpha"));
}

#[test]
fn skills_exclude_writes_to_project_ace_toml() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts6", &["alpha"]);

    env.ace().args(["skills", "exclude", "alpha"]).assert().success();

    let toml = std::fs::read_to_string(env.path("ace.toml")).expect("read ace.toml");
    assert!(toml.contains("exclude_skills"), "missing exclude_skills key:\n{toml}");
}

#[test]
fn skills_include_dedups_within_scope() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts7", &["alpha"]);

    env.ace().args(["skills", "include", "alpha"]).assert().success();
    env.ace().args(["skills", "include", "alpha"]).assert().success();

    let toml = std::fs::read_to_string(env.path("ace.toml")).expect("read ace.toml");
    let count = toml.matches("\"alpha\"").count();
    assert_eq!(count, 1, "alpha should appear once after dedup:\n{toml}");
}

#[test]
fn skills_reset_drops_both_lists() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts8", &["alpha", "beta"]);

    env.ace().args(["skills", "include", "alpha"]).assert().success();
    env.ace().args(["skills", "exclude", "beta"]).assert().success();
    env.ace().args(["skills", "reset"]).assert().success();

    let toml = std::fs::read_to_string(env.path("ace.toml")).expect("read ace.toml");
    assert!(!toml.contains("include_skills"), "include_skills should be reset:\n{toml}");
    assert!(!toml.contains("exclude_skills"), "exclude_skills should be reset:\n{toml}");
}

#[test]
fn skills_reset_include_only() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts9", &["alpha", "beta"]);

    env.ace().args(["skills", "include", "alpha"]).assert().success();
    env.ace().args(["skills", "exclude", "beta"]).assert().success();
    env.ace().args(["skills", "reset", "--include"]).assert().success();

    let toml = std::fs::read_to_string(env.path("ace.toml")).expect("read ace.toml");
    assert!(!toml.contains("include_skills"));
    assert!(toml.contains("exclude_skills"), "exclude_skills should remain:\n{toml}");
}

#[test]
fn skills_user_scope_writes_to_user_ace_toml() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts10", &["alpha"]);

    env.ace().args(["--user", "skills", "include", "alpha"]).assert().success();

    // Project ace.toml should NOT have include_skills
    let proj = std::fs::read_to_string(env.path("ace.toml")).expect("read project ace.toml");
    assert!(!proj.contains("include_skills"), "project ace.toml should be untouched:\n{proj}");

    // User-scope file lives under XDG_CONFIG_HOME/ace/ace.toml; TestEnv sets that to <root>/config.
    let user = std::fs::read_to_string(env.path("config/ace/ace.toml"))
        .expect("read user ace.toml");
    assert!(user.contains("include_skills"));
    assert!(user.contains("alpha"));
}

#[test]
fn skills_invalid_pattern_is_rejected() {
    // `?` is still rejected; `**` is now spec-accepted (treated as `*`)
    // per docs/spec/skills/selection.md § Glob patterns. Use `?` here
    // to keep covering the validator's reject path.
    let env = TestEnv::new();
    setup_school_with_skills(&env, "ts11", &["alpha"]);

    let output = env.ace()
        .args(["skills", "include", "alpha?"])
        .output()
        .expect("ace skills include alpha?");
    assert!(!output.status.success(), "should reject `?`");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid pattern") || stderr.contains("?"),
        "expected validation error in stderr:\n{stderr}");
}
