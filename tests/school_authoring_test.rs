mod common;

use common::TestEnv;
use predicates::prelude::*;

// Authoring commands resolve cwd-first: a school.toml in the working directory
// wins over the linked school, even when `ace.toml` points elsewhere.
// Contract: docs/spec/school/school-commands.md (resolution table).

#[test]
fn authoring_prefers_cwd_school_over_linked() {
    let env = TestEnv::new();
    env.setup_remote_school("acme/school");
    env.write_file("school.toml", "name = \"cwd-school\"\n");
    env.mkdir("skills/homebase");
    env.write_file("skills/homebase/SKILL.md", "# Homebase\n");

    // Divergence: cwd school and linked school both exist — cwd wins,
    // linked skills stay untouched, and no fallback warning fires.
    env.ace()
        .args(["school", "skills"])
        .assert()
        .success()
        .stdout(predicates::str::contains("homebase"))
        .stdout(predicates::str::contains("maverick").not())
        .stderr(predicates::str::contains("linked school").not());
}

#[test]
fn authoring_falls_back_to_linked_school_with_warning() {
    let env = TestEnv::new();
    env.setup_remote_school("acme/school");

    // No cwd school.toml — fall back to the linked school, announced.
    env.ace()
        .args(["school", "skills"])
        .assert()
        .success()
        .stdout(predicates::str::contains("maverick"))
        .stderr(predicates::str::contains("linked school"));
}

#[test]
fn authoring_without_any_school_names_both_paths_out() {
    let env = TestEnv::new();
    env.git_init();

    // Neither cwd school.toml nor a resolvable specifier — hard error whose
    // hint names both bootstrap routes.
    env.ace()
        .args(["school", "skills"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("ace school init"))
        .stderr(predicates::str::contains("ace setup"));
}
