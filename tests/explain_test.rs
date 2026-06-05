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
fn explain_active_skill_shows_trace() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "te1", &["alpha"]);

    let output = env
        .ace()
        .args(["explain", "alpha"])
        .output()
        .expect("ace explain alpha");
    assert!(output.status.success(), "ace explain should succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("alpha"));
    assert!(stdout.contains("status: active"));
    assert!(stdout.contains("base"));
}

#[test]
fn explain_excluded_skill_shows_removal() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "te2", &["alpha", "beta"]);

    env.ace()
        .args(["skills", "exclude", "alpha"])
        .assert()
        .success();

    let output = env
        .ace()
        .args(["explain", "alpha"])
        .output()
        .expect("ace explain alpha");
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("status: excluded"));
    assert!(stdout.contains("removed"));
    assert!(stdout.contains("exclude_skills"));
}

#[test]
fn explain_unknown_skill_suggests_near_matches() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "te3", &["rust-coding", "rust-fmt"]);

    let output = env
        .ace()
        .args(["explain", "rust-cod"])
        .output()
        .expect("ace explain rust-cod");
    assert!(!output.status.success(), "unknown skill should fail");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown skill"));
    assert!(
        stderr.contains("rust-coding"),
        "should suggest near match:\n{stderr}"
    );
}

#[test]
fn explain_unknown_no_overlap_just_errors() {
    let env = TestEnv::new();
    setup_school_with_skills(&env, "te4", &["alpha"]);

    let output = env
        .ace()
        .args(["explain", "xz"])
        .output()
        .expect("ace explain xz");
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unknown skill"));
}
