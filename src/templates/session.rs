use std::collections::HashMap;
use std::path::Path;

use crate::templates::builtins;
use crate::templates::Template;
use crate::actions::project::{ChangeKind, SkillChange};

/// Build the session prompt from built-in + school + project layers.
pub fn build_session_prompt(
    school_name: &str,
    school_session_prompt: &str,
    project_session_prompt: &str,
    backend_dir: &Path,
    changes: &[SkillChange],
    school_clone: Option<&Path>,
    school_is_dirty: bool,
    excluded_skills: &[String],
) -> String {
    let mut parts = Vec::new();

    let vals = HashMap::from([
        ("school_name".to_string(), school_name.to_string()),
    ]);
    parts.push(Template::parse(builtins::SESSION).substitute(&vals));

    if !school_session_prompt.is_empty() {
        parts.push(school_session_prompt.to_string());
    }

    if !project_session_prompt.is_empty() {
        parts.push(project_session_prompt.to_string());
    }

    if !excluded_skills.is_empty() {
        let mut sorted: Vec<String> = excluded_skills.to_vec();
        sorted.sort();
        let names = sorted
            .iter()
            .map(|n| format!("- `{n}`"))
            .collect::<Vec<_>>()
            .join("\n");
        let vals = HashMap::from([("names".to_string(), names)]);
        parts.push(Template::parse(builtins::EXCLUDED_SKILLS).substitute(&vals));
    }

    if !changes.is_empty() {
        parts.push(format_change_summary(changes));
    }

    if let Some(clone_path) = school_clone {
        let vals = HashMap::from([
            ("school_clone".to_string(), clone_path.display().to_string()),
        ]);
        parts.push(Template::parse(builtins::SCHOOL_CHANGES).substitute(&vals));

        if school_is_dirty {
            parts.push(builtins::DIRTY_SCHOOL.to_string());
        }
    }

    let previous_skills = backend_dir.join("previous-skills");
    if previous_skills.exists() {
        let backend_dir_name = backend_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(".claude");
        let vals = HashMap::from([
            ("backend_dir".to_string(), backend_dir_name.to_string()),
        ]);
        parts.push(Template::parse(builtins::PREVIOUS_SKILLS).substitute(&vals));
    }

    parts.iter()
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_change_summary(changes: &[SkillChange]) -> String {
    let mut added: Vec<&str> = Vec::new();
    let mut modified: Vec<&str> = Vec::new();
    let mut removed: Vec<&str> = Vec::new();

    for c in changes {
        match c.kind {
            ChangeKind::Added => added.push(&c.name),
            ChangeKind::Modified => modified.push(&c.name),
            ChangeKind::Removed => removed.push(&c.name),
        }
    }

    let mut lines = Vec::new();
    for (label, names) in [("Added", added), ("Updated", modified), ("Removed", removed)] {
        for name in names {
            lines.push(format!("- {label}: `{name}`"));
        }
    }

    let vals = HashMap::from([
        ("changes".to_string(), lines.join("\n")),
    ]);
    Template::parse(builtins::CHANGES).substitute(&vals)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nonexistent_dir() -> std::path::PathBuf {
        let tmp = tempfile::tempdir().expect("tempdir");
        tmp.path().join("ace-test-prompt-nonexistent")
    }

    struct TempDir(tempfile::TempDir);

    impl TempDir {
        fn new(_name: &str) -> Self {
            Self(tempfile::tempdir().expect("tempdir"))
        }

        fn path(&self) -> &Path {
            self.0.path()
        }
    }

    #[test]
    fn builtin_includes_session_base() {
        let dir = nonexistent_dir();
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], None, false, &[]);
        assert!(prompt.contains("School: Acme"));
        assert!(prompt.contains("ACE (Accelerated Coding Environment)"));
    }

    #[test]
    fn no_cache_omits_proposal_steps() {
        let dir = nonexistent_dir();
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], None, false, &[]);
        assert!(!prompt.contains("School clone:"));
        assert!(!prompt.contains("propose school changes"));
    }

    #[test]
    fn school_and_project_prompts() {
        let dir = nonexistent_dir();
        let prompt = build_session_prompt("Acme", "Use Rust.", "PostgreSQL project.", &dir, &[], None, false, &[]);
        assert!(prompt.contains("Use Rust."));
        assert!(prompt.contains("PostgreSQL project."));
        assert_layer_order(&prompt, "Use Rust.", "PostgreSQL project.");
    }

    #[test]
    fn skips_empty_layers() {
        let dir = nonexistent_dir();
        let prompt = build_session_prompt("Acme", "", "Only project.", &dir, &[], None, false, &[]);
        assert!(prompt.contains("Only project."));
    }

    #[test]
    fn injects_previous_skills_guidance() {
        let fix = TempDir::new("ace-test-prompt-previous");
        let prev = fix.path().join("previous-skills");
        std::fs::create_dir_all(&prev).expect("create previous-skills dir");

        let prompt = build_session_prompt("Acme", "", "", fix.path(), &[], None, false, &[]);
        assert!(prompt.contains("unconsolidated skills"));
        assert!(prompt.contains("previous-skills"));
    }

    #[test]
    fn previous_skills_uses_backend_dir_name() {
        let fix = TempDir::new("ace-test-prompt-agents");
        let skills = fix.path().join(".agents").join("previous-skills");
        std::fs::create_dir_all(&skills).expect("create previous-skills dir");

        let backend_dir = fix.path().join(".agents");
        let prompt = build_session_prompt("Acme", "", "", &backend_dir, &[], None, false, &[]);
        assert!(prompt.contains(".agents/previous-skills/"), "should use .agents dir name");
        assert!(!prompt.contains(".claude/previous-skills/"), "should not contain .claude");
    }

    #[test]
    fn no_previous_skills_no_injection() {
        let fix = TempDir::new("ace-test-prompt-no-previous");
        std::fs::create_dir_all(fix.path().join("skills")).expect("create skills dir");

        let prompt = build_session_prompt("Acme", "", "", fix.path(), &[], None, false, &[]);
        assert!(!prompt.contains("unconsolidated"));
    }

    #[test]
    fn injects_change_summary() {
        let dir = nonexistent_dir();
        let changes = vec![
            SkillChange { name: "new-skill".into(), kind: ChangeKind::Added },
            SkillChange { name: "existing".into(), kind: ChangeKind::Modified },
            SkillChange { name: "old-skill".into(), kind: ChangeKind::Removed },
        ];

        let prompt = build_session_prompt("Acme", "", "", &dir, &changes, None, false, &[]);
        assert!(prompt.contains("School skills were updated"));
        assert!(prompt.contains("- Added: `new-skill`"));
        assert!(prompt.contains("- Updated: `existing`"));
        assert!(prompt.contains("- Removed: `old-skill`"));
    }

    #[test]
    fn no_changes_no_summary() {
        let dir = nonexistent_dir();
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], None, false, &[]);
        assert!(!prompt.contains("updated since your last session"));
    }

    #[test]
    fn injects_school_clone_and_proposal_steps() {
        let dir = nonexistent_dir();
        let clone_path = Path::new("/home/user/.local/share/ace/org/school");
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], Some(clone_path), false, &[]);
        assert!(prompt.contains("School clone: /home/user/.local/share/ace/org/school"));
        assert!(prompt.contains("guide the user through"));
        assert!(prompt.contains("git -C /home/user/.local/share/ace/org/school"));
    }

    fn sample_changes() -> Vec<SkillChange> {
        vec![
            SkillChange { name: "new-skill".into(), kind: ChangeKind::Added },
            SkillChange { name: "existing".into(), kind: ChangeKind::Modified },
        ]
    }

    fn assert_layer_order(prompt: &str, earlier: &str, later: &str) {
        let a = prompt.find(earlier).unwrap_or_else(|| panic!("missing: {earlier}"));
        let b = prompt.find(later).unwrap_or_else(|| panic!("missing: {later}"));
        assert!(a < b, "expected '{earlier}' before '{later}'");
    }

    #[test]
    fn changes_and_cache() {
        let dir = nonexistent_dir();
        let clone_path = Path::new("/tmp/school");
        let prompt = build_session_prompt("Acme", "", "", &dir, &sample_changes(), Some(clone_path), false, &[]);
        assert!(prompt.contains("School skills were updated"));
        assert!(prompt.contains("School clone:"));
        assert_layer_order(&prompt, "School skills were updated", "School clone:");
    }

    #[test]
    fn changes_and_previous_skills() {
        let fix = TempDir::new("ace-test-changes-prev");
        std::fs::create_dir_all(fix.path().join("previous-skills")).expect("mkdir");

        let prompt = build_session_prompt("Acme", "", "", fix.path(), &sample_changes(), None, false, &[]);
        assert!(prompt.contains("School skills were updated"));
        assert!(prompt.contains("unconsolidated skills"));
        assert_layer_order(&prompt, "School skills were updated", "unconsolidated skills");
    }

    #[test]
    fn cache_and_previous_skills() {
        let fix = TempDir::new("ace-test-cache-prev");
        std::fs::create_dir_all(fix.path().join("previous-skills")).expect("mkdir");
        let clone_path = Path::new("/tmp/school");

        let prompt = build_session_prompt("Acme", "", "", fix.path(), &[], Some(clone_path), false, &[]);
        assert!(prompt.contains("School clone:"));
        assert!(prompt.contains("unconsolidated skills"));
        assert_layer_order(&prompt, "School clone:", "unconsolidated skills");
    }

    #[test]
    fn changes_cache_and_previous_skills() {
        let fix = TempDir::new("ace-test-all-optional");
        std::fs::create_dir_all(fix.path().join("previous-skills")).expect("mkdir");
        let clone_path = Path::new("/tmp/school");

        let prompt = build_session_prompt("Acme", "", "", fix.path(), &sample_changes(), Some(clone_path), false, &[]);
        assert!(prompt.contains("School skills were updated"));
        assert!(prompt.contains("School clone:"));
        assert!(prompt.contains("unconsolidated skills"));
        assert_layer_order(&prompt, "School skills were updated", "School clone:");
        assert_layer_order(&prompt, "School clone:", "unconsolidated skills");
    }

    #[test]
    fn all_layers_present() {
        let fix = TempDir::new("ace-test-all-layers");
        std::fs::create_dir_all(fix.path().join("previous-skills")).expect("mkdir");
        let clone_path = Path::new("/tmp/school");

        let prompt = build_session_prompt(
            "Acme", "School rules.", "Project rules.",
            fix.path(), &sample_changes(), Some(clone_path), false, &[],
        );

        assert!(prompt.contains("School: Acme"));
        assert!(prompt.contains("School rules."));
        assert!(prompt.contains("Project rules."));
        assert!(prompt.contains("School skills were updated"));
        assert!(prompt.contains("School clone:"));
        assert!(prompt.contains("unconsolidated skills"));

        assert_layer_order(&prompt, "School: Acme", "School rules.");
        assert_layer_order(&prompt, "School rules.", "Project rules.");
        assert_layer_order(&prompt, "Project rules.", "School skills were updated");
        assert_layer_order(&prompt, "School skills were updated", "School clone:");
        assert_layer_order(&prompt, "School clone:", "unconsolidated skills");
    }

    #[test]
    fn no_optional_layers() {
        let dir = nonexistent_dir();
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], None, false, &[]);
        assert!(prompt.contains("School: Acme"));
        assert!(!prompt.contains("School skills were updated"));
        assert!(!prompt.contains("School clone:"));
        assert!(!prompt.contains("unconsolidated"));
    }

    #[test]
    fn dirty_school_includes_gitignore_guidance() {
        let dir = nonexistent_dir();
        let clone_path = Path::new("/tmp/school");
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], Some(clone_path), true, &[]);
        assert!(prompt.contains("uncommitted local changes"));
        assert!(prompt.contains(".gitignore"));
    }

    #[test]
    fn clean_school_omits_dirty_notice() {
        let dir = nonexistent_dir();
        let clone_path = Path::new("/tmp/school");
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], Some(clone_path), false, &[]);
        assert!(!prompt.contains("uncommitted local changes"));
    }

    #[test]
    fn dirty_school_layer_order() {
        let dir = nonexistent_dir();
        let clone_path = Path::new("/tmp/school");
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], Some(clone_path), true, &[]);
        assert_layer_order(&prompt, "School clone:", "uncommitted local changes");
    }

    #[test]
    fn excluded_skills_section_appears_with_names() {
        let dir = nonexistent_dir();
        let excluded = vec!["foo".to_string(), "bar".to_string()];
        let prompt = build_session_prompt(
            "Acme", "", "", &dir, &[], None, false, &excluded,
        );
        assert!(prompt.contains("Excluded skills"));
        assert!(prompt.contains("- `foo`"));
        assert!(prompt.contains("- `bar`"));
    }

    #[test]
    fn excluded_skills_empty_omits_section() {
        let dir = nonexistent_dir();
        let prompt = build_session_prompt("Acme", "", "", &dir, &[], None, false, &[]);
        assert!(!prompt.contains("Excluded skills"));
    }

    #[test]
    fn excluded_skills_names_sorted() {
        let dir = nonexistent_dir();
        let excluded = vec!["zebra".to_string(), "apple".to_string(), "mango".to_string()];
        let prompt = build_session_prompt(
            "Acme", "", "", &dir, &[], None, false, &excluded,
        );
        let a = prompt.find("`apple`").expect("apple");
        let m = prompt.find("`mango`").expect("mango");
        let z = prompt.find("`zebra`").expect("zebra");
        assert!(a < m && m < z, "expected sorted order apple < mango < zebra");
    }

    #[test]
    fn excluded_skills_layer_order() {
        let fix = TempDir::new("ace-test-excluded-order");
        std::fs::create_dir_all(fix.path().join("previous-skills")).expect("mkdir");
        let clone_path = Path::new("/tmp/school");
        let excluded = vec!["foo".to_string()];
        let prompt = build_session_prompt(
            "Acme", "School rules.", "Project rules.",
            fix.path(), &sample_changes(), Some(clone_path), false, &excluded,
        );
        // Sits after project_session_prompt, before changes.
        assert_layer_order(&prompt, "Project rules.", "Excluded skills");
        assert_layer_order(&prompt, "Excluded skills", "School skills were updated");
        assert_layer_order(&prompt, "School skills were updated", "School clone:");
        assert_layer_order(&prompt, "School clone:", "unconsolidated skills");
    }
}
