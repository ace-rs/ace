#[path = "../common/fixtures.rs"]
mod common;

use common::TestEnv;

use crate::ace::{Ace, Io};
use crate::actions::project::edit_config::{EditConfig, FieldEdit};
use crate::config::paths::AcePaths;

fn instance(env: &TestEnv) -> Ace {
    Ace::new(
        env.root().to_path_buf(),
        AcePaths {
            user: env.path("config/ace/ace.toml"),
            project: env.path("ace.toml"),
            local: env.path("ace.local.toml"),
            cache: env.path("cache"),
        },
        Io::new(true, true),
    )
}

#[test]
fn edits_refresh_config_backend_and_linked_school_on_the_same_instance() {
    let env = TestEnv::new();
    env.write_file("ace.toml", "school = \".:first\"\nbackend = \"codex\"\n");
    env.write_file("first/school.toml", "name = \"first\"\n");
    env.write_file("second/school.toml", "name = \"second\"\n");
    let mut ace = instance(&env);
    assert_eq!(ace.backend().expect("initial backend").name, "codex");
    assert_eq!(ace.school().expect("initial school").name, "first");
    let path = env.path("ace.toml");

    EditConfig {
        path: &path,
        assignments: vec![
            FieldEdit::new("backend", "claude"),
            FieldEdit::new("school", ".:second"),
        ],
    }
    .run(&mut ace)
    .expect("publish edits");

    assert_eq!(
        ace.require_config().expect("new config").backend_name.value,
        "claude"
    );
    assert_eq!(ace.backend().expect("new backend").name, "claude");
    assert_eq!(ace.school().expect("new school").name, "second");
    assert_eq!(
        ace.require_linked_school().expect("new location").root,
        env.path("second")
    );
}

#[test]
fn invalid_assignment_preserves_the_file_and_cached_configuration() {
    let env = TestEnv::new();
    let original = "# retained\nbackend = \"codex\"\n";
    env.write_file("ace.toml", original);
    let mut ace = instance(&env);
    assert_eq!(
        ace.require_config()
            .expect("initial config")
            .backend_name
            .value,
        "codex"
    );
    let path = env.path("ace.toml");

    let result = EditConfig {
        path: &path,
        assignments: vec![FieldEdit::new("backend", false)],
    }
    .run(&mut ace);

    assert!(
        result.is_err(),
        "invalid field type must not reach publication"
    );
    assert_eq!(env.read_file("ace.toml"), original);
    assert_eq!(
        ace.require_config()
            .expect("retained config")
            .backend_name
            .value,
        "codex"
    );
}
