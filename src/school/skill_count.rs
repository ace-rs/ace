//! Skill-count helper for the `ace learn` auto-trigger.
//!
//! When the school exposes more skills than a project realistically needs,
//! `ace setup`, `ace school pull-imports`, and `ace` startup all offer to
//! run `ace learn` to narrow the `skills` filter. The threshold is a
//! hardcoded constant — convention over configuration.

use std::path::Path;

use crate::ace::Ace;
use crate::actions::project::learn::LearnAction;
use crate::cmd::CmdError;
use crate::skills::{ChangeKind, SkillChange};

/// Skill count above which the auto-trigger fires.
const LEARN_THRESHOLD: usize = 10;

/// Discover the resolved school's skill count. Returns 0 on any error
/// (no school, discovery I/O failure) — callers treat that as "no
/// trigger" rather than surfacing the error.
pub fn count(ace: &Ace) -> usize {
    let Ok(paths) = ace.require_school() else {
        return 0;
    };
    let Ok(skills) = crate::skills::Skills::<crate::skills::Discovered>::discover(&paths.root)
    else {
        return 0;
    };
    skills.names().count()
}

/// Peek at the project's `ace.toml` and check whether the user has
/// explicitly set the `skills` key. Distinguishes `skills = []` (opted out)
/// from a missing key (never set) — these collapse to the same in-memory
/// `Vec<String>` after serde, so we read the raw TOML.
///
/// Returns `false` when the file doesn't exist or is unreadable.
pub fn has_explicit_skills_key(project_ace_toml: &Path) -> bool {
    let Ok(content) = std::fs::read_to_string(project_ace_toml) else {
        return false;
    };
    let Ok(value) = toml::from_str::<toml::Value>(&content) else {
        return false;
    };
    value.get("skills").is_some()
}

/// Inline y/N prompt — common shape used by setup, pull-imports, and ace
/// startup. Skips silently in non-Human (porcelain) mode and when the
/// user already pinned `skills` in ace.toml.
///
/// On `y`, runs `LearnAction` immediately — the inline prompt here
/// is the confirm; the action itself does no prompting.
pub fn maybe_offer_learn(ace: &mut Ace) -> Result<(), CmdError> {
    use crate::ace::OutputMode;

    if ace.mode() != OutputMode::Human {
        return Ok(());
    }

    let count = count(ace);
    if count <= LEARN_THRESHOLD {
        return Ok(());
    }

    let project_ace_toml = ace.project_dir().join("ace.toml");
    if has_explicit_skills_key(&project_ace_toml) {
        return Ok(());
    }

    let prompt = format!(
        "school has {count} skills — run `ace learn` now to narrow to what this project needs?"
    );
    if !ace.prompt_confirm(&prompt, false)? {
        return Ok(());
    }

    LearnAction.run(ace).map_err(CmdError::from)
}

/// Soft hint when the school's skill set just changed and the user already
/// has an explicit `skills` filter — they pinned it deliberately, so don't
/// re-prompt; just nudge that the world moved.
///
/// Skip conditions: no changes, count under threshold, no explicit `skills`
/// filter (the inline prompt path handles that), or non-Human output mode.
pub fn maybe_hint_relearn(ace: &mut Ace, changes: &[SkillChange]) {
    use crate::ace::OutputMode;

    if ace.mode() != OutputMode::Human || changes.is_empty() {
        return;
    }
    if count(ace) <= LEARN_THRESHOLD {
        return;
    }
    if !has_explicit_skills_key(&ace.project_dir().join("ace.toml")) {
        return;
    }

    let (added, removed) = tally_changes(changes);
    ace.hint(&format!(
        "school skills changed ({added}+/{removed}-) — consider rerunning `ace learn`"
    ));
}

fn tally_changes(changes: &[SkillChange]) -> (usize, usize) {
    let mut added = 0;
    let mut removed = 0;
    for c in changes {
        match c.kind {
            ChangeKind::Added => added += 1,
            ChangeKind::Removed => removed += 1,
            ChangeKind::Modified => {}
        }
    }
    (added, removed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_skills_key_present_with_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "school = \"x\"\nskills = [\"a\", \"b\"]\n").unwrap();
        assert!(has_explicit_skills_key(&path));
    }

    #[test]
    fn explicit_skills_key_present_when_empty_array() {
        // `skills = []` is the explicit opt-out — must register as "set".
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "school = \"x\"\nskills = []\n").unwrap();
        assert!(has_explicit_skills_key(&path));
    }

    #[test]
    fn explicit_skills_key_absent_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "school = \"x\"\n").unwrap();
        assert!(!has_explicit_skills_key(&path));
    }

    #[test]
    fn explicit_skills_key_absent_when_only_comments() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "# skills = [\"a\"]\nschool = \"x\"\n").unwrap();
        assert!(!has_explicit_skills_key(&path));
    }

    #[test]
    fn explicit_skills_key_absent_when_file_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.toml");
        assert!(!has_explicit_skills_key(&path));
    }

    #[test]
    fn tally_change_kinds_independently() {
        let changes = [
            SkillChange { name: "a".into(), kind: ChangeKind::Added },
            SkillChange { name: "b".into(), kind: ChangeKind::Added },
            SkillChange { name: "c".into(), kind: ChangeKind::Removed },
            SkillChange { name: "d".into(), kind: ChangeKind::Modified },
        ];
        assert_eq!(tally_changes(&changes), (2, 1));
    }

    #[test]
    fn explicit_skills_key_absent_when_unparseable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.toml");
        std::fs::write(&path, "not valid {{{ toml").unwrap();
        assert!(!has_explicit_skills_key(&path));
    }
}
