//! Per-skill symlink reconciliation.
//!
//! Replaces the legacy whole-dir `<backend>/skills` symlink. The skills
//! directory becomes a real dir; each enabled skill gets its own symlink
//! pointing into the school clone. Re-runs reconcile in place: add, repoint,
//! remove ACE-managed links to match the desired set; warn on foreign entries.
//!
//! ACE-managed predicate: a symlink whose target resolves textually inside
//! the school clone's `skills/` subtree. No marker files.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::ace::Ace;
use crate::actions::project::link::LinkResult;
use crate::config::tree::Tree;
use crate::skills::{Decided, Skills};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredLink {
    pub name: String,
    pub target: PathBuf,
}

/// Snapshot of one entry currently inside `<backend>/skills/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentEntry {
    pub name: String,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntryKind {
    /// Symlink whose target resolves inside `school_skills_root` — safe to manage.
    ManagedSymlink { target: PathBuf },
    /// Symlink with a target outside the school skills root — leave alone.
    ForeignSymlink { target: PathBuf },
    /// Real file or directory placed by the user — leave alone.
    ForeignEntry,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAction {
    Create { name: String, target: PathBuf },
    Repoint { name: String, target: PathBuf },
    Remove { name: String },
    SkipForeign { name: String, reason: String },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkPlan {
    pub actions: Vec<LinkAction>,
}

/// Compute the reconciliation plan. Pure: no I/O.
pub fn plan(desired: &[DesiredLink], current: &[CurrentEntry]) -> LinkPlan {
    let desired_names: HashSet<&str> = desired.iter().map(|d| d.name.as_str()).collect();

    let actions_for_desired = desired.iter().filter_map(|want| {
        let existing = current.iter().find(|c| c.name == want.name);
        decide_action(want, existing)
    });

    let actions_for_orphans = current.iter().filter_map(|entry| {
        if desired_names.contains(entry.name.as_str()) {
            return None;
        }
        match entry.kind {
            EntryKind::ManagedSymlink { .. } => Some(LinkAction::Remove {
                name: entry.name.clone(),
            }),
            // Foreign orphans: leave alone, no warning needed.
            EntryKind::ForeignSymlink { .. } | EntryKind::ForeignEntry => None,
        }
    });

    LinkPlan {
        actions: actions_for_desired.chain(actions_for_orphans).collect(),
    }
}

/// Decide what to do with one desired link given the current state of that
/// name's entry. `None` means "already correct, no action needed."
fn decide_action(want: &DesiredLink, existing: Option<&CurrentEntry>) -> Option<LinkAction> {
    let Some(entry) = existing else {
        return Some(LinkAction::Create {
            name: want.name.clone(),
            target: want.target.clone(),
        });
    };
    match &entry.kind {
        EntryKind::ManagedSymlink { target } if target == &want.target => None,
        EntryKind::ManagedSymlink { .. } => Some(LinkAction::Repoint {
            name: want.name.clone(),
            target: want.target.clone(),
        }),
        EntryKind::ForeignSymlink { target } => Some(LinkAction::SkipForeign {
            name: want.name.clone(),
            reason: format!("symlink points outside school clone: {}", target.display()),
        }),
        EntryKind::ForeignEntry => Some(LinkAction::SkipForeign {
            name: want.name.clone(),
            reason: "not managed by ace (file or directory exists)".to_string(),
        }),
    }
}

/// Classify a directory entry. Reads the symlink target if applicable;
/// pure given the input string slices (no further I/O).
pub fn classify(name: &str, kind_input: ClassifyInput, school_skills_root: &Path) -> CurrentEntry {
    let kind = match kind_input {
        ClassifyInput::Symlink(target) => {
            if target.starts_with(school_skills_root) {
                EntryKind::ManagedSymlink { target }
            } else {
                EntryKind::ForeignSymlink { target }
            }
        }
        ClassifyInput::Other => EntryKind::ForeignEntry,
    };
    CurrentEntry {
        name: name.to_string(),
        kind,
    }
}

/// Pulled out so `classify` stays pure. The I/O wrapper packages disk reads
/// into one of these variants.
pub enum ClassifyInput {
    Symlink(PathBuf),
    Other,
}

/// Discover + resolve + map included skills to `(name, path)` pairs.
///
/// Walks the school's `skills/` tree, resolves against the three config
/// layers, and emits one `DesiredLink` per included skill. Path comes
/// straight from `Skill<Decided>` — no separate name→path join.
pub fn prepare(school_root: &Path, tree: &Tree) -> io::Result<PreparedSkills> {
    let skills = Skills::discover(school_root)?.resolve(tree);
    let desired = skills
        .included()
        .map(|s| DesiredLink {
            name: s.name.clone(),
            target: s.path.clone(),
        })
        .collect();
    Ok(PreparedSkills { desired, skills })
}

#[derive(Debug)]
pub struct PreparedSkills {
    pub desired: Vec<DesiredLink>,
    pub skills: Skills<Decided>,
}

/// Reconcile per-skill symlinks under `project_skills_dir`.
///
/// - Migrates the legacy whole-dir symlink (if `project_skills_dir` is itself
///   a symlink, unlink it) and ensures `project_skills_dir` is a real dir.
/// - Reads current entries, classifies vs `school_skills_root`, plans, executes.
/// - Returns reconciliation summary including warnings for foreign entries.
pub fn reconcile(
    school_skills_root: &Path,
    project_skills_dir: &Path,
    desired: &[DesiredLink],
) -> io::Result<ReconcileResult> {
    if is_symlink(project_skills_dir) {
        std::fs::remove_file(project_skills_dir)?;
    }
    std::fs::create_dir_all(project_skills_dir)?;

    let current = scan_current(project_skills_dir, school_skills_root)?;
    let plan = plan(desired, &current);

    let mut result = ReconcileResult::default();
    for action in &plan.actions {
        match action {
            LinkAction::Create { name, target } => {
                create_dir_symlink(target, &project_skills_dir.join(name))?;
                result.created += 1;
            }
            LinkAction::Repoint { name, target } => {
                let path = project_skills_dir.join(name);
                fs::remove_file(&path)?;
                create_dir_symlink(target, &path)?;
                result.repointed += 1;
            }
            LinkAction::Remove { name } => {
                fs::remove_file(project_skills_dir.join(name))?;
                result.removed += 1;
            }
            LinkAction::SkipForeign { name, reason } => {
                result.warnings.push(format!("cannot link {name}: {reason}"));
            }
        }
    }
    Ok(result)
}

/// Emit user-visible warnings for resolution diagnostics + link warnings.
/// Shared by all callers that run the prepare → reconcile sequence.
pub fn emit_warnings(ace: &mut Ace, prepared: &PreparedSkills, link_result: &LinkResult) {
    for warning in &link_result.skill_warnings {
        ace.warn(warning);
    }
    let diagnostics = prepared.skills.diagnostics();
    for unknown in &diagnostics.unknown_patterns {
        ace.warn(&format!(
            "skill pattern matched no skill: {} (in {:?} {:?})",
            unknown.pattern, unknown.source, unknown.field
        ));
    }
    for collision in &diagnostics.collisions {
        ace.warn(&format!(
            "skill {} appears in both include_skills and exclude_skills at {:?} scope",
            collision.skill, collision.source
        ));
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct ReconcileResult {
    pub created: usize,
    pub repointed: usize,
    pub removed: usize,
    pub warnings: Vec<String>,
}

impl ReconcileResult {
    pub fn changed(&self) -> bool {
        self.created + self.repointed + self.removed > 0
    }
}

fn scan_current(
    project_skills_dir: &Path,
    school_skills_root: &Path,
) -> io::Result<Vec<CurrentEntry>> {
    // Canonicalize both sides so the prefix check in `classify` isn't fooled
    // by symlinked path components (e.g. macOS `/var` → `/private/var`, or a
    // school root reached through a parent symlink). Broken links fall back
    // to the raw read_link target — they won't match the canonical root and
    // classify as foreign, which is the safe default for a stale link.
    let canonical_root = fs::canonicalize(school_skills_root)?;
    let mut out = Vec::new();
    for entry in fs::read_dir(project_skills_dir)? {
        let entry = entry?;
        let name = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let path = entry.path();
        let kind_input = if is_symlink(&path) {
            let resolved = fs::canonicalize(&path).or_else(|_| fs::read_link(&path))?;
            ClassifyInput::Symlink(resolved)
        } else {
            ClassifyInput::Other
        };
        out.push(classify(&name, kind_input, &canonical_root));
    }
    Ok(out)
}

pub(super) fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Create a directory-level symlink. Platform-split: Unix uses `symlink`;
/// Windows uses `symlink_dir` (directory symlinks don't require admin).
pub(super) fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desired(pairs: &[(&str, &str)]) -> Vec<DesiredLink> {
        pairs
            .iter()
            .map(|(n, t)| DesiredLink {
                name: (*n).to_string(),
                target: PathBuf::from(*t),
            })
            .collect()
    }

    fn managed(name: &str, target: &str) -> CurrentEntry {
        CurrentEntry {
            name: name.to_string(),
            kind: EntryKind::ManagedSymlink {
                target: PathBuf::from(target),
            },
        }
    }

    fn foreign_link(name: &str, target: &str) -> CurrentEntry {
        CurrentEntry {
            name: name.to_string(),
            kind: EntryKind::ForeignSymlink {
                target: PathBuf::from(target),
            },
        }
    }

    fn foreign_entry(name: &str) -> CurrentEntry {
        CurrentEntry {
            name: name.to_string(),
            kind: EntryKind::ForeignEntry,
        }
    }

    #[test]
    fn empty_dir_creates_all_desired() {
        let p = plan(&desired(&[("a", "/sch/a"), ("b", "/sch/b")]), &[]);
        assert_eq!(
            p.actions,
            vec![
                LinkAction::Create { name: "a".into(), target: "/sch/a".into() },
                LinkAction::Create { name: "b".into(), target: "/sch/b".into() },
            ]
        );
    }

    #[test]
    fn correct_managed_link_is_left_alone() {
        let p = plan(
            &desired(&[("a", "/sch/a")]),
            &[managed("a", "/sch/a")],
        );
        assert!(p.actions.is_empty());
    }

    #[test]
    fn stale_managed_link_is_repointed() {
        let p = plan(
            &desired(&[("a", "/sch/a-new")]),
            &[managed("a", "/sch/a-old")],
        );
        assert_eq!(
            p.actions,
            vec![LinkAction::Repoint { name: "a".into(), target: "/sch/a-new".into() }]
        );
    }

    #[test]
    fn orphaned_managed_link_is_removed() {
        let p = plan(
            &desired(&[("b", "/sch/b")]),
            &[managed("a", "/sch/a"), managed("b", "/sch/b")],
        );
        assert_eq!(p.actions, vec![LinkAction::Remove { name: "a".into() }]);
    }

    #[test]
    fn foreign_symlink_is_skipped_with_reason() {
        let p = plan(
            &desired(&[("a", "/sch/a")]),
            &[foreign_link("a", "/elsewhere")],
        );
        assert_eq!(p.actions.len(), 1);
        assert!(matches!(p.actions[0], LinkAction::SkipForeign { .. }));
        if let LinkAction::SkipForeign { reason, .. } = &p.actions[0] {
            assert!(reason.contains("/elsewhere"));
        }
    }

    #[test]
    fn foreign_real_entry_is_skipped() {
        let p = plan(
            &desired(&[("a", "/sch/a")]),
            &[foreign_entry("a")],
        );
        assert_eq!(p.actions.len(), 1);
        assert!(matches!(p.actions[0], LinkAction::SkipForeign { .. }));
    }

    #[test]
    fn foreign_orphan_is_left_alone() {
        // User dropped a real dir for a skill we don't link — no action, no warn.
        let p = plan(&desired(&[]), &[foreign_entry("user-stuff")]);
        assert!(p.actions.is_empty());
    }

    #[test]
    fn classify_managed_when_target_inside_root() {
        let entry = classify(
            "a",
            ClassifyInput::Symlink(PathBuf::from("/sch/skills/a")),
            Path::new("/sch/skills"),
        );
        assert_eq!(
            entry.kind,
            EntryKind::ManagedSymlink { target: PathBuf::from("/sch/skills/a") }
        );
    }

    #[test]
    fn classify_foreign_when_target_outside_root() {
        let entry = classify(
            "a",
            ClassifyInput::Symlink(PathBuf::from("/elsewhere/a")),
            Path::new("/sch/skills"),
        );
        assert!(matches!(entry.kind, EntryKind::ForeignSymlink { .. }));
    }

    #[test]
    fn classify_other_is_foreign_entry() {
        let entry = classify("a", ClassifyInput::Other, Path::new("/sch/skills"));
        assert_eq!(entry.kind, EntryKind::ForeignEntry);
    }

    #[test]
    #[cfg(unix)]
    fn scan_current_classifies_managed_link_through_symlinked_school_root() {
        // Regression: textual `PathBuf::starts_with` misclassifies a managed
        // link as foreign when the school root path goes through a parent
        // symlink (or any non-canonical form).
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();

        let real_school = root.join("real_school");
        let real_skills = real_school.join("skills");
        let real_skill = real_skills.join("foo");
        std::fs::create_dir_all(&real_skill).expect("real skill dir");

        let linked_school = root.join("linked_school");
        std::os::unix::fs::symlink(&real_school, &linked_school).expect("symlink school");
        let linked_skills = linked_school.join("skills");

        let project_skills = root.join("proj").join("skills");
        std::fs::create_dir_all(&project_skills).expect("proj skills");
        std::os::unix::fs::symlink(&real_skill, project_skills.join("foo"))
            .expect("managed symlink");

        let entries = scan_current(&project_skills, &linked_skills).expect("scan");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "foo");
        assert!(
            matches!(entries[0].kind, EntryKind::ManagedSymlink { .. }),
            "expected ManagedSymlink through symlinked school root, got {:?}",
            entries[0].kind,
        );
    }

    #[test]
    fn mixed_scenario() {
        // desired: a (new), b (correct), c (repoint)
        // current: b (correct), c (stale), d (orphan-managed), foo (orphan-foreign)
        let p = plan(
            &desired(&[("a", "/sch/a"), ("b", "/sch/b"), ("c", "/sch/c-new")]),
            &[
                managed("b", "/sch/b"),
                managed("c", "/sch/c-old"),
                managed("d", "/sch/d"),
                foreign_entry("foo"),
            ],
        );
        assert_eq!(
            p.actions,
            vec![
                LinkAction::Create { name: "a".into(), target: "/sch/a".into() },
                LinkAction::Repoint { name: "c".into(), target: "/sch/c-new".into() },
                LinkAction::Remove { name: "d".into() },
            ]
        );
    }
}
