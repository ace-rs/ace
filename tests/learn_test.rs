mod common;

use common::TestEnv;

/// Set up an embedded school with N skills (test-skill, plus extras).
fn setup_school_with_skill_count(env: &TestEnv, extras: usize) {
    env.git_init();
    env.write_file("school.toml", "name = \"test-school\"\n");
    env.write_file("ace.toml", "school = \".\"\nbackend = \"flaude\"\n");
    env.write_file("CLAUDE.md", "# Test\n");
    env.mkdir(".claude");
    env.symlink("skills", ".claude/skills");

    env.mkdir("skills/test-skill");
    env.write_file("skills/test-skill/SKILL.md", "# Test\n");

    for i in 0..extras {
        let dir = format!("skills/extra-{i}");
        env.mkdir(&dir);
        env.write_file(&format!("{dir}/SKILL.md"), &format!("# Extra {i}\n"));
    }
}

#[test]
fn learn_writes_skills_to_ace_toml_from_stdout() {
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 5);

    // Backend "agent" returns: keep test-skill + a glob.
    env.ace()
        .args(["learn"])
        .env("FLAUDE_ONE_SHOT_STDOUT", "test-skill\nextra-*\n")
        .assert()
        .success();

    let ace_toml = env.read_file("ace.toml");
    assert!(
        ace_toml.contains("test-skill") && ace_toml.contains("extra-*"),
        "ace.toml should contain agent-selected skills, got:\n{ace_toml}",
    );
    // Hardcoded ACE skills always present.
    assert!(
        ace_toml.contains("\"ace\"") && ace_toml.contains("\"ace-*\""),
        "ace.toml must include hardcoded ace skills, got:\n{ace_toml}",
    );

    // Verify the one-shot was actually invoked.
    let one_shot = env.read_flaude_one_shot_records();
    assert_eq!(one_shot.len(), 1, "should record one exec_one_shot");
    assert_eq!(one_shot[0].prompt_kind.as_deref(), Some("inline"));
    let prompt = one_shot[0].prompt_text.as_deref().unwrap_or("");
    assert!(
        prompt.contains("test-skill") && prompt.contains("Available skills"),
        "prompt should embed available skill list",
    );
}

#[test]
fn learn_drops_unknown_literals_with_warning() {
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 2);

    let output = env
        .ace()
        .args(["learn"])
        .env("FLAUDE_ONE_SHOT_STDOUT", "test-skill\nbogus-name\n")
        .output()
        .expect("ace learn");

    assert!(output.status.success(), "ace learn should succeed");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("bogus-name") && stderr.contains("unknown skill"),
        "should warn on unknown skill, stderr:\n{stderr}",
    );

    let ace_toml = env.read_file("ace.toml");
    assert!(
        ace_toml.contains("test-skill"),
        "kept skills should be in ace.toml: {ace_toml}",
    );
    assert!(
        !ace_toml.contains("bogus-name"),
        "dropped skills should not be in ace.toml: {ace_toml}",
    );
}

#[test]
fn learn_proceeds_in_non_human_mode_without_prompt() {
    // In non-Human (porcelain/test) mode there's no TTY to prompt on —
    // explicit `ace learn` invocation is consent enough.
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 2);

    env.ace()
        .args(["learn"])
        .env("FLAUDE_ONE_SHOT_STDOUT", "test-skill\n")
        // No stdin — would block if a prompt fired.
        .assert()
        .success();

    let ace_toml = env.read_file("ace.toml");
    assert!(
        ace_toml.contains("test-skill"),
        "ace.toml should have rewritten skills: {ace_toml}",
    );
}

#[test]
fn learn_propagates_backend_nonzero_exit() {
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 2);

    let output = env
        .ace()
        .args(["learn"])
        .env("FLAUDE_ONE_SHOT_EXIT_CODE", "7")
        .env("FLAUDE_ONE_SHOT_STDERR", "model crashed\n")
        .output()
        .expect("ace learn");

    assert!(!output.status.success(), "ace learn should fail on backend non-zero");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("model crashed") || stderr.contains("backend exited"),
        "stderr should surface backend failure, got:\n{stderr}",
    );

    // ace.toml must NOT have been rewritten on backend failure.
    let ace_toml = env.read_file("ace.toml");
    assert!(
        !ace_toml.contains("skills"),
        "ace.toml must be untouched on backend failure: {ace_toml}",
    );
}

#[test]
fn learn_with_empty_stdout_writes_empty_skills() {
    // Spec: empty result is acceptable (small school, novel project).
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 2);

    env.ace()
        .args(["learn"])
        .env("FLAUDE_ONE_SHOT_STDOUT", "")
        .assert()
        .success();

    // Even with empty agent output, hardcoded ACE skills are present.
    let ace_toml = env.read_file("ace.toml");
    assert!(
        ace_toml.contains("\"ace\"") && ace_toml.contains("\"ace-*\""),
        "empty agent output should still include hardcoded ace skills: {ace_toml}",
    );
}

#[test]
fn learn_replaces_existing_skills_array_not_appends() {
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 5);
    // Pre-pin a different filter; learn must replace, not merge.
    env.write_file(
        "ace.toml",
        "school = \".\"\nbackend = \"flaude\"\nskills = [\"old-skill\"]\n",
    );

    env.ace()
        .args(["learn"])
        .env("FLAUDE_ONE_SHOT_STDOUT", "test-skill\nextra-1\n")
        .assert()
        .success();

    let ace_toml = env.read_file("ace.toml");
    assert!(
        !ace_toml.contains("old-skill"),
        "learn must REPLACE the skills array, not append: {ace_toml}",
    );
    assert!(
        ace_toml.contains("test-skill") && ace_toml.contains("extra-1"),
        "new skills must be present: {ace_toml}",
    );
}

#[test]
fn learn_preserves_other_ace_toml_keys() {
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 2);
    env.write_file(
        "ace.toml",
        "school = \".\"\nbackend = \"flaude\"\nsession_prompt = \"hello there\"\n\n[env]\nFOO = \"bar\"\n",
    );

    env.ace()
        .args(["learn"])
        .env("FLAUDE_ONE_SHOT_STDOUT", "test-skill\n")
        .assert()
        .success();

    let ace_toml = env.read_file("ace.toml");
    assert!(ace_toml.contains("session_prompt"), "session_prompt preserved: {ace_toml}");
    assert!(ace_toml.contains("hello there"), "session_prompt value preserved");
    assert!(ace_toml.contains("FOO") && ace_toml.contains("bar"), "[env] block preserved");
    assert!(ace_toml.contains("test-skill"), "skills written");
}

#[test]
fn auto_trigger_skipped_when_skills_explicit_even_empty() {
    // `skills = []` is the explicit opt-out — auto-trigger must skip.
    // Use `ace pull` so the relearn-hint path is also exercised; here we
    // only assert no auto-spend.
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"big\"\n");
    env.write_file(
        "ace.toml",
        "school = \".\"\nbackend = \"flaude\"\nskills = []\n",
    );
    for i in 0..15 {
        let dir = format!("skills/extra-{i}");
        env.mkdir(&dir);
        env.write_file(&format!("{dir}/SKILL.md"), &format!("# Extra {i}\n"));
    }
    env.write_file("CLAUDE.md", "# Test\n");
    env.mkdir(".claude");
    env.symlink("skills", ".claude/skills");

    env.ace()
        .args(["--porcelain"])
        .env("ACE_SKIP_UPDATE", "1")
        .assert()
        .success();

    assert!(
        env.read_flaude_one_shot_records().is_empty(),
        "auto-trigger must respect explicit skills = [] opt-out",
    );
}

#[test]
fn learn_dedupes_repeated_lines_in_stdout() {
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 3);

    env.ace()
        .args(["learn"])
        .env(
            "FLAUDE_ONE_SHOT_STDOUT",
            "test-skill\ntest-skill\nextra-1\ntest-skill\n",
        )
        .assert()
        .success();

    let ace_toml = env.read_file("ace.toml");
    let occurrences = ace_toml.matches("\"test-skill\"").count();
    assert_eq!(occurrences, 1, "duplicates must be deduped, got:\n{ace_toml}");
}

#[test]
fn learn_strips_decoration_in_agent_output() {
    // Real LLMs leak fences and bullets — parser must tolerate.
    let env = TestEnv::new();
    setup_school_with_skill_count(&env, 3);

    env.ace()
        .args(["learn"])
        .env(
            "FLAUDE_ONE_SHOT_STDOUT",
            "```\n- test-skill\n* `extra-1`\nextra-*\n```\n",
        )
        .assert()
        .success();

    let ace_toml = env.read_file("ace.toml");
    for expected in ["test-skill", "extra-1", "extra-*"] {
        assert!(
            ace_toml.contains(expected),
            "missing {expected} after strip; got:\n{ace_toml}",
        );
    }
    assert!(!ace_toml.contains("```"), "fences must not leak through: {ace_toml}");
}

#[test]
fn setup_above_threshold_does_not_auto_spend_in_porcelain() {
    // Auto-trigger must skip silently in non-Human mode — `ace setup` should
    // never spend tokens just because the school is large.
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"big\"\n");
    for i in 0..15 {
        let dir = format!("skills/extra-{i}");
        env.mkdir(&dir);
        env.write_file(&format!("{dir}/SKILL.md"), &format!("# Extra {i}\n"));
    }

    env.ace()
        .args(["setup", "."])
        .env("ACE_SKIP_UPDATE", "1")
        .assert()
        .success();

    assert!(
        env.read_flaude_one_shot_records().is_empty(),
        "auto-trigger must not spend tokens in non-Human mode",
    );
}

#[test]
fn setup_does_not_offer_learn_below_threshold() {
    // 5 skills (1 + 4 extras) — under the 10-skill threshold, no offer.
    let env = TestEnv::new();
    env.git_init();
    env.write_file("school.toml", "name = \"small\"\n");
    env.mkdir("skills/test-skill");
    env.write_file("skills/test-skill/SKILL.md", "# Test\n");
    for i in 0..4 {
        let dir = format!("skills/extra-{i}");
        env.mkdir(&dir);
        env.write_file(&format!("{dir}/SKILL.md"), &format!("# Extra {i}\n"));
    }

    let output = env
        .ace()
        .args(["setup", "."])
        .env("ACE_SKIP_UPDATE", "1")
        .output()
        .expect("ace setup");

    assert!(output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stderr.contains("ace learn") && !stdout.contains("ace learn"),
        "no learn offer below threshold; combined output:\n{stderr}\n{stdout}",
    );
}
