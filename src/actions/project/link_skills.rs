//! Per-skill symlink reconciliation.
//!
//! Replaces the legacy whole-dir `<backend>/skills` symlink. The skills
//! directory becomes a real dir; each enabled skill gets its own symlink
//! pointing into the school clone. Re-runs reconcile in place: add, repoint,
//! remove ACE-managed links to match the desired set; warn on foreign entries.
//!
//! ACE-managed predicate: a symlink whose target resolves textually inside
//! either the current school root OR the ACE data root
//! (`~/.local/share/ace/`, parent of all school clones). The data-root check
//! catches stragglers from a previous `school = "..."` value pointing into a
//! sibling clone; the school-root check covers embedded schools
//! (`school = "."`) whose root sits outside the data root. No marker files.

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
    /// Symlink whose target resolves inside a managed root — safe to manage.
    ManagedSymlink { target: PathBuf },
    /// Symlink with a target outside every managed root — leave alone.
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
            reason: format!(
                "symlink points outside ace-managed roots: {}",
                target.display()
            ),
        }),
        EntryKind::ForeignEntry => Some(LinkAction::SkipForeign {
            name: want.name.clone(),
            reason: "not managed by ace (file or directory exists)".to_string(),
        }),
    }
}

/// Classify a directory entry. Reads the symlink target if applicable;
/// pure given the input string slices (no further I/O).
///
/// A symlink is managed if its target sits under any of `managed_roots`.
/// Production callers pass the current school root plus the ACE data root;
/// tests may pass a single root.
pub fn classify(name: &str, kind_input: ClassifyInput, managed_roots: &[&Path]) -> CurrentEntry {
    let kind = match kind_input {
        ClassifyInput::Symlink(target) => {
            if managed_roots.iter().any(|r| target.starts_with(r)) {
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
/// layers, and emits one `DesiredLink` per included skill. The link
/// name follows the backend-emit rule from `docs/spec/skills/emit.md`:
/// `basename(identity)`, structurally checked at the boundary. The path is the
/// only naming axis (`docs/decisions/2026-06-01-skill-name-is-path.md`).
///
/// When two included skills produce the same backend dirname, the
/// loser is dropped per spec § Loser-drop on collision (alphabetical
/// by source path tiebreaker) and a warning is recorded.
pub fn prepare(
    school_root: &Path,
    tree: &Tree,
    backend_features: u32,
) -> io::Result<PreparedSkills> {
    let (validated, rejected) = Skills::discover(school_root)?.validate();
    let skills = validated.resolve(tree).with_rejected(rejected);
    let (desired, collision_warnings) = build_desired(skills.included(), backend_features);
    Ok(PreparedSkills {
        desired,
        skills,
        collision_warnings,
    })
}

/// Map an iterator of included skills to a deduplicated `DesiredLink`
/// list, applying the capability-driven backend emit rule:
///
/// - Nested-capable backend (`FEATURE_NESTED_SKILLS` set) AND identity
///   depth ≤ `MAX_SKILL_DEPTH` → emit verbatim at the identity path, no
///   collision check (paths are unique in school storage).
/// - Otherwise → flatten branch: `skillName = basename(identity)`, structural
///   check, alphabetical-by-source-path tiebreak, loser-drop + warn on collision.
///
/// See `docs/spec/skills/emit.md` § Backend emit rule.
fn build_desired<'a, I>(included: I, backend_features: u32) -> (Vec<DesiredLink>, Vec<String>)
where
    I: Iterator<Item = &'a crate::skills::Skill<crate::skills::Decided>>,
{
    use crate::backend::{FEATURE_NESTED_SKILLS, MAX_SKILL_DEPTH};

    let nested_capable = backend_features & FEATURE_NESTED_SKILLS != 0;
    let mut warnings: Vec<String> = Vec::new();

    // Split skills by branch. Nested-emit skills carry their identity path
    // verbatim and never collide (paths are unique in school storage). The
    // remainder go through the flatten branch with structural validation +
    // alphabetical tiebreak + loser-drop per `emit.md` § Loser-drop on collision.
    let mut nested: Vec<DesiredLink> = Vec::new();
    let mut to_flatten: Vec<&crate::skills::Skill<crate::skills::Decided>> = Vec::new();
    for skill in included {
        let identity = skill.locator.as_str();
        let depth = identity.split('/').count();
        if nested_capable && depth <= MAX_SKILL_DEPTH {
            if let Err(reason) = structural_path_ok(identity) {
                warnings.push(format!(
                    "skill `{}` produces unsafe backend path ({reason}); dropping",
                    crate::skills::name::render(identity),
                ));
                continue;
            }
            nested.push(DesiredLink {
                name: identity.to_string(),
                target: skill.path.clone(),
            });
        } else {
            to_flatten.push(skill);
        }
    }

    let mut candidates: Vec<(String, &Path, &str)> = to_flatten
        .iter()
        .map(|s| {
            let link_name = basename_of(s.locator.as_str()).to_string();
            (link_name, s.path.as_path(), s.locator.as_str())
        })
        .collect();
    candidates.sort_by(|a, b| a.2.cmp(b.2));

    let mut by_link: std::collections::HashMap<String, (PathBuf, &str)> =
        std::collections::HashMap::new();

    for (link_name, path, identity) in candidates {
        if let Err(reason) = crate::skills::name::structural_ok(
            &link_name,
            crate::skills::name::NameContext::BackendLinkName,
        ) {
            warnings.push(format!(
                "skill `{}` produces unsafe backend name ({reason}); dropping",
                crate::skills::name::render(identity),
            ));
            continue;
        }
        if let Some((_, winner_identity)) = by_link.get(&link_name) {
            warnings.push(format!(
                "backend-name collision at `{link_name}`: `{winner_identity}` wins over \
                 `{identity}` (alphabetical-by-source-path). Loser is dropped from the backend. \
                 Two identities share a leaf on a flat backend — restructure the skill paths or \
                 use `[[imports]]` `exclude_skills` to express disjoint sets.",
            ));
            continue;
        }
        by_link.insert(link_name.clone(), (path.to_path_buf(), identity));
    }

    let mut desired: Vec<DesiredLink> = nested;
    desired.extend(
        by_link
            .into_iter()
            .map(|(name, (target, _))| DesiredLink { name, target }),
    );
    desired.sort_by(|a, b| a.name.cmp(&b.name));
    (desired, warnings)
}

/// Last path segment of a slash-joined identity. For flat identities
/// returns the whole identity; for nested returns the leaf.
fn basename_of(identity: &str) -> &str {
    identity.rsplit('/').next().unwrap_or(identity)
}

fn structural_path_ok(identity: &str) -> Result<(), crate::skills::name::RejectReason> {
    for segment in identity.split('/') {
        crate::skills::name::structural_ok(
            segment,
            crate::skills::name::NameContext::BackendLinkName,
        )?;
    }
    Ok(())
}

#[derive(Debug)]
pub struct PreparedSkills {
    pub desired: Vec<DesiredLink>,
    pub skills: Skills<Decided>,
    /// Warnings produced by the backend emit rule: structural rejection,
    /// dirname collisions, etc. Surfaced via `emit_warnings`.
    pub collision_warnings: Vec<String>,
}

/// Reconcile per-skill symlinks under `project_skills_dir`.
///
/// - Migrates the legacy whole-dir symlink (if `project_skills_dir` is itself
///   a symlink, unlink it) and ensures `project_skills_dir` is a real dir.
/// - Reads current entries, classifies against `school_root` + `ace_data_root`,
///   plans, executes.
/// - Returns reconciliation summary including warnings for foreign entries.
///
/// `school_root` covers the current school clone (and the project itself for
/// embedded `school = "."`). `ace_data_root` (`~/.local/share/ace/`) covers
/// leftover per-skill links pointing at a sibling clone — so switching schools
/// via `ace.toml` prunes those on the next link/setup. If `ace_data_root`
/// doesn't exist on disk (e.g. no schools have ever been cloned), it's
/// silently dropped from the predicate.
pub fn reconcile(
    school_root: &Path,
    ace_data_root: &Path,
    project_skills_dir: &Path,
    desired: &[DesiredLink],
) -> io::Result<ReconcileResult> {
    if is_symlink(project_skills_dir) {
        std::fs::remove_file(project_skills_dir)?;
    }
    std::fs::create_dir_all(project_skills_dir)?;

    let current = scan_current(project_skills_dir, school_root, ace_data_root)?;
    let plan = plan(desired, &current);

    let mut result = ReconcileResult::default();
    for action in &plan.actions {
        match action {
            LinkAction::Create { name, target } => {
                let link = project_skills_dir.join(name);
                if let Some(parent) = link.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                create_dir_symlink(target, &link)?;
                result.created += 1;
            }
            LinkAction::Repoint { name, target } => {
                let path = project_skills_dir.join(name);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                fs::remove_file(&path)?;
                create_dir_symlink(target, &path)?;
                result.repointed += 1;
            }
            LinkAction::Remove { name } => {
                let path = project_skills_dir.join(name);
                fs::remove_file(&path)?;
                prune_empty_ancestors(&path, project_skills_dir);
                result.removed += 1;
            }
            LinkAction::SkipForeign { name, reason } => {
                result
                    .warnings
                    .push(format!("cannot link {name}: {reason}"));
            }
        }
    }
    Ok(result)
}

/// Walk up from `removed_link` toward `stop` (exclusive), removing any
/// directory along the way that is now empty. Stops on first non-empty
/// directory or on read errors; never deletes `stop` itself.
fn prune_empty_ancestors(removed_link: &Path, stop: &Path) {
    let mut cur = removed_link.parent();
    while let Some(dir) = cur {
        if dir == stop {
            return;
        }
        match fs::read_dir(dir) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return;
                }
            }
            Err(_) => return,
        }
        if fs::remove_dir(dir).is_err() {
            return;
        }
        cur = dir.parent();
    }
}

/// Emit user-visible warnings for resolution diagnostics + link warnings.
/// Shared by all callers that run the prepare → reconcile sequence.
pub fn emit_warnings(ace: &mut Ace, prepared: &PreparedSkills, link_result: &LinkResult) {
    for warning in &link_result.skill_warnings {
        ace.warn(warning);
    }
    for warning in &prepared.collision_warnings {
        ace.warn(warning);
    }
    for rejected in prepared.skills.rejected() {
        ace.warn(&format!(
            "skill `{}` rejected: {}",
            crate::skills::name::render(rejected.locator.as_str()),
            rejected.reason,
        ));
    }
    let diagnostics = prepared.skills.diagnostics();
    for unknown in &diagnostics.unknown_patterns {
        ace.warn(&format!(
            "skill pattern matched no skill: {} (in {:?} {:?})",
            crate::skills::name::render(&unknown.pattern),
            unknown.source,
            unknown.field,
        ));
    }
    for collision in &diagnostics.collisions {
        ace.warn(&format!(
            "skill {} appears in both include_skills and exclude_skills at {:?} scope",
            crate::skills::name::render(&collision.skill),
            collision.source,
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
    school_root: &Path,
    ace_data_root: &Path,
) -> io::Result<Vec<CurrentEntry>> {
    // Canonicalize each managed root so the prefix check in `classify` isn't
    // fooled by symlinked path components (e.g. macOS `/var` → `/private/var`,
    // or a school root reached through a parent symlink). `school_root` is
    // the active source of skills, so a missing path here is a hard error.
    // `ace_data_root` is optional: embedded schools may run before any school
    // has been cloned, leaving the data dir non-existent — in that case it
    // drops from the predicate. Broken project-side links fall back to the
    // raw read_link target; they won't match any canonical root and classify
    // as foreign, which is the safe default for a stale link.
    let canonical_school = fs::canonicalize(school_root)?;
    let canonical_data = fs::canonicalize(ace_data_root).ok();
    let mut roots: Vec<&Path> = vec![&canonical_school];
    if let Some(data) = &canonical_data {
        roots.push(data);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(project_skills_dir)? {
        let entry = entry?;
        let name = match entry.file_name().to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let path = entry.path();
        if is_symlink(&path) {
            let resolved = fs::canonicalize(&path).or_else(|_| fs::read_link(&path))?;
            out.push(classify(&name, ClassifyInput::Symlink(resolved), &roots));
        } else if path.is_dir() {
            // Tentatively descend: a real dir that contains only managed
            // symlinks (or other such dirs) is an ACE-managed nested
            // parent; emit each managed link as its own entry. If
            // descent finds any non-managed content, the whole dir is
            // treated as a ForeignEntry instead — preserving the
            // existing left-alone behavior for user-placed dirs.
            let mut nested = Vec::new();
            let foreign = collect_nested(&path, project_skills_dir, &roots, &mut nested)?;
            if foreign || nested.is_empty() {
                out.push(classify(&name, ClassifyInput::Other, &roots));
            } else {
                out.extend(nested);
            }
        } else {
            out.push(classify(&name, ClassifyInput::Other, &roots));
        }
    }
    Ok(out)
}

/// Walk `dir` recursively. Append every symlink as a CurrentEntry rooted
/// at `project_skills_dir`. Return `true` if any non-managed content
/// (foreign symlink, real file, etc.) is found — the caller treats the
/// whole top-level dir as a ForeignEntry in that case.
fn collect_nested(
    dir: &Path,
    project_skills_dir: &Path,
    roots: &[&Path],
    out: &mut Vec<CurrentEntry>,
) -> io::Result<bool> {
    let mut foreign = false;
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let rel = match path.strip_prefix(project_skills_dir) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let name = match rel.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        if is_symlink(&path) {
            let resolved = fs::canonicalize(&path).or_else(|_| fs::read_link(&path))?;
            let entry = classify(&name, ClassifyInput::Symlink(resolved), roots);
            if matches!(entry.kind, EntryKind::ManagedSymlink { .. }) {
                out.push(entry);
            } else {
                foreign = true;
            }
        } else if path.is_dir() {
            if collect_nested(&path, project_skills_dir, roots, out)? {
                foreign = true;
            }
        } else {
            foreign = true;
        }
    }
    Ok(foreign)
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
                LinkAction::Create {
                    name: "a".into(),
                    target: "/sch/a".into()
                },
                LinkAction::Create {
                    name: "b".into(),
                    target: "/sch/b".into()
                },
            ]
        );
    }

    #[test]
    fn correct_managed_link_is_left_alone() {
        let p = plan(&desired(&[("a", "/sch/a")]), &[managed("a", "/sch/a")]);
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
            vec![LinkAction::Repoint {
                name: "a".into(),
                target: "/sch/a-new".into()
            }]
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
        let p = plan(&desired(&[("a", "/sch/a")]), &[foreign_entry("a")]);
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
            &[Path::new("/sch/skills")],
        );
        assert_eq!(
            entry.kind,
            EntryKind::ManagedSymlink {
                target: PathBuf::from("/sch/skills/a")
            }
        );
    }

    #[test]
    fn classify_foreign_when_target_outside_root() {
        let entry = classify(
            "a",
            ClassifyInput::Symlink(PathBuf::from("/elsewhere/a")),
            &[Path::new("/sch/skills")],
        );
        assert!(matches!(entry.kind, EntryKind::ForeignSymlink { .. }));
    }

    #[test]
    fn classify_other_is_foreign_entry() {
        let entry = classify("a", ClassifyInput::Other, &[Path::new("/sch/skills")]);
        assert_eq!(entry.kind, EntryKind::ForeignEntry);
    }

    #[test]
    fn classify_managed_when_target_inside_sibling_school_clone() {
        // The fix: a symlink pointing into a sibling school clone (left over
        // from a previous `school = "..."` value) classifies as managed when
        // the ACE data root is among the managed roots.
        let entry = classify(
            "ghost",
            ClassifyInput::Symlink(PathBuf::from("/data/ace/old-owner/old-repo/skills/ghost")),
            &[
                Path::new("/data/ace/new-owner/new-repo"),
                Path::new("/data/ace"),
            ],
        );
        assert!(
            matches!(entry.kind, EntryKind::ManagedSymlink { .. }),
            "expected ManagedSymlink for sibling clone, got {:?}",
            entry.kind,
        );
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

        let entries = scan_current(&project_skills, &linked_skills, &linked_skills).expect("scan");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "foo");
        assert!(
            matches!(entries[0].kind, EntryKind::ManagedSymlink { .. }),
            "expected ManagedSymlink through symlinked school root, got {:?}",
            entries[0].kind,
        );
    }

    // -- backend emit rule (spec: emit.md § Backend emit rule) --

    use crate::skills::discover::Tier;
    use crate::skills::{Decided, Decision, Locator, Skill};

    fn included_skill(identity: &str, path: &str) -> Skill<Decided> {
        Skill {
            locator: Locator::from_basename(identity),
            path: PathBuf::from(path),
            tier: Tier::Curated,
            internal: false,
            frontmatter_name: None,
            source: None,
            state: Decided {
                decision: Decision::Included,
                trace: Vec::new(),
            },
        }
    }

    #[test]
    fn flat_identity_link_name_equals_basename() {
        let skills = [included_skill("rust-coding", "/s/rust-coding")];
        let (desired, warnings) = build_desired(skills.iter(), 0);
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].name, "rust-coding");
        assert!(warnings.is_empty());
    }

    #[test]
    fn nested_identity_link_name_uses_leaf() {
        // <school>/skills/typescript/coding/ → backend link `coding`.
        let skills = [included_skill("typescript/coding", "/s/typescript/coding")];
        let (desired, _) = build_desired(skills.iter(), 0);
        assert_eq!(desired[0].name, "coding");
    }

    #[test]
    fn collision_drops_loser_alphabetically() {
        // Two nested skills produce the same leaf `coding`. Alphabetical
        // by source path: `python/coding` wins over `typescript/coding`.
        let skills = [
            included_skill("typescript/coding", "/s/typescript/coding"),
            included_skill("python/coding", "/s/python/coding"),
        ];
        let (desired, warnings) = build_desired(skills.iter(), 0);
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].name, "coding");
        assert_eq!(desired[0].target, PathBuf::from("/s/python/coding"));
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("collision"));
        assert!(warnings[0].contains("python/coding"));
        assert!(warnings[0].contains("typescript/coding"));
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
                LinkAction::Create {
                    name: "a".into(),
                    target: "/sch/a".into()
                },
                LinkAction::Repoint {
                    name: "c".into(),
                    target: "/sch/c-new".into()
                },
                LinkAction::Remove { name: "d".into() },
            ]
        );
    }

    // -- capability-driven emit (spec: emit.md § Backend emit rule) --

    use crate::backend::FEATURE_NESTED_SKILLS;

    #[test]
    fn nested_capable_emits_verbatim() {
        // FEATURE_NESTED_SKILLS set: identity path preserved as link name,
        // no flatten.
        let skills = [included_skill("typescript/coding", "/s/typescript/coding")];
        let (desired, warnings) = build_desired(skills.iter(), FEATURE_NESTED_SKILLS);
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].name, "typescript/coding");
        assert!(warnings.is_empty());
    }

    #[test]
    fn flat_backend_flattens_identity() {
        // features=0: existing behavior — leaf name only.
        let skills = [included_skill("typescript/coding", "/s/typescript/coding")];
        let (desired, _) = build_desired(skills.iter(), 0);
        assert_eq!(desired[0].name, "coding");
    }

    #[test]
    fn depth_cap_falls_through_to_flatten() {
        // 6-segment identity exceeds MAX_SKILL_DEPTH=5 → flatten branch
        // even with FEATURE_NESTED_SKILLS set.
        let skills = [included_skill("a/b/c/d/e/f", "/s/a/b/c/d/e/f")];
        let (desired, _) = build_desired(skills.iter(), FEATURE_NESTED_SKILLS);
        assert_eq!(desired[0].name, "f");
    }

    #[test]
    fn nested_emit_skips_collision_check() {
        // Two skills share a leaf `foo` at different identity paths. On a
        // nested-capable backend both emit verbatim — no collision, no warning.
        let skills = [
            included_skill("a/foo", "/s/a/foo"),
            included_skill("b/foo", "/s/b/foo"),
        ];
        let (mut desired, warnings) = build_desired(skills.iter(), FEATURE_NESTED_SKILLS);
        desired.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(desired.len(), 2);
        assert_eq!(desired[0].name, "a/foo");
        assert_eq!(desired[1].name, "b/foo");
        assert!(warnings.is_empty());
    }

    #[test]
    fn mixed_depth_routes_per_skill() {
        // Depth 3 (≤ MAX_SKILL_DEPTH) emits nested; depth 6 falls through
        // to flatten as leaf — same emit, per-skill router.
        let skills = [
            included_skill("a/b/c", "/s/a/b/c"),
            included_skill("a/b/c/d/e/f", "/s/a/b/c/d/e/f"),
        ];
        let (mut desired, _) = build_desired(skills.iter(), FEATURE_NESTED_SKILLS);
        desired.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(desired.len(), 2);
        assert_eq!(desired[0].name, "a/b/c");
        assert_eq!(desired[0].target, PathBuf::from("/s/a/b/c"));
        assert_eq!(desired[1].name, "f");
        assert_eq!(desired[1].target, PathBuf::from("/s/a/b/c/d/e/f"));
    }

    // -- emit structural backstop (spec: emit.md § Backend-emit writes) --
    //
    // The emit name is `basename(identity)` — a single path segment by
    // construction, so slash / multi-level traversal threats are neutralized
    // by the basename split, not by a check. What survives is a leaf that is
    // itself structurally unsafe (leading dot, bare dot-segment, NUL, length,
    // backslash). Character admission is discovery's job; emit re-checks
    // structure only, as a filesystem-edge backstop independent of admission.

    #[test]
    fn emit_does_not_mutate_chars() {
        // Emit is a structural backstop, not a character gate — it never
        // rewrites Unicode content. A bidi char in the leaf passes structure
        // and emits verbatim; rejecting it is discovery-admission's job.
        let skills = [included_skill(
            "good\u{202E}coding",
            "/s/good\u{202E}coding",
        )];
        let (desired, warnings) = build_desired(skills.iter(), 0);
        assert_eq!(desired[0].name, "good\u{202E}coding");
        assert!(warnings.is_empty());
    }

    #[test]
    fn nul_in_leaf_warns_and_drops() {
        let skills = [included_skill("foo\0bar", "/s/foo")];
        let (desired, warnings) = build_desired(skills.iter(), 0);
        assert!(desired.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("NUL"));
    }

    #[test]
    fn dot_segment_leaf_warns_and_drops() {
        // A leaf that is bare `.` or `..` traverses: `<skills>/.` is the dir
        // itself; `<skills>/..` is the parent. Chars pass admission; structure
        // does not.
        for spoof in [".", ".."] {
            let skills = [included_skill(spoof, "/s/foo")];
            let (desired, warnings) = build_desired(skills.iter(), 0);
            assert!(desired.is_empty(), "{spoof:?} should be dropped");
            assert_eq!(warnings.len(), 1);
            assert!(
                warnings[0].contains("dot segment"),
                "warning was: {}",
                warnings[0]
            );
        }
    }

    #[test]
    fn leading_dot_leaf_warns_and_drops() {
        // `.gitignore`, `.env`, etc. — would shadow real dotfiles in the
        // backend skills dir. Drop defensively.
        for spoof in [".gitignore", ".env", ".ssh", ".hidden"] {
            let skills = [included_skill(spoof, "/s/foo")];
            let (desired, warnings) = build_desired(skills.iter(), 0);
            assert!(desired.is_empty(), "{spoof:?} should be dropped");
            assert!(
                warnings[0].contains("starts with a dot"),
                "warning was: {}",
                warnings[0]
            );
        }
    }

    #[test]
    fn oversized_leaf_warns_and_drops() {
        // Filesystem per-component cap is 255 bytes; reject earlier.
        let huge = "a".repeat(300);
        let skills = [included_skill(&huge, "/s/foo")];
        let (desired, warnings) = build_desired(skills.iter(), 0);
        assert!(desired.is_empty());
        assert!(warnings[0].contains("255"), "warning was: {}", warnings[0]);
    }

    #[test]
    fn backslash_in_leaf_warns_and_drops() {
        // Backslash is a legal filename char on unix but a path separator
        // on Windows. Reject defensively — symbol with no legitimate use
        // in a flat-backend skill name.
        let skills = [included_skill("foo\\bar", "/s/foo")];
        let (desired, warnings) = build_desired(skills.iter(), 0);
        assert!(desired.is_empty());
        assert!(
            warnings[0].contains("contains `\\`"),
            "warning was: {}",
            warnings[0]
        );
    }

    #[test]
    fn slash_in_identity_on_nested_branch_is_legitimate() {
        // Identity-path `/` is the whole point of the nested branch — it
        // emits verbatim, slashes intact.
        let skills = [included_skill("typescript/coding", "/s/typescript/coding")];
        let (desired, warnings) = build_desired(skills.iter(), FEATURE_NESTED_SKILLS);
        assert_eq!(desired.len(), 1);
        assert_eq!(desired[0].name, "typescript/coding");
        assert!(warnings.is_empty());
    }

    // -- capability-driven emit: reconcile (nested layout) --

    #[test]
    #[cfg(unix)]
    fn reconcile_creates_parent_dirs_for_nested_link() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let school_skills = root.join("school").join("skills");
        let nested_target = school_skills.join("typescript").join("coding");
        std::fs::create_dir_all(&nested_target).expect("nested skill dir");

        let project_skills = root.join("proj").join("skills");

        let desired = vec![DesiredLink {
            name: "typescript/coding".to_string(),
            target: nested_target.clone(),
        }];
        reconcile(&school_skills, &school_skills, &project_skills, &desired).expect("reconcile");

        let link = project_skills.join("typescript").join("coding");
        assert!(is_symlink(&link), "expected nested symlink at {link:?}");
        let resolved = std::fs::canonicalize(&link).expect("canonicalize");
        assert_eq!(
            resolved,
            std::fs::canonicalize(&nested_target).expect("target")
        );
    }

    #[test]
    #[cfg(unix)]
    fn reconcile_repoints_existing_nested_link() {
        // Seed a managed symlink at a nested path → reconcile with same
        // name + new target → assert Repoint re-pointed via the
        // create_dir_all(parent) path, with the parent dir intact.
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let school_skills = root.join("school").join("skills");
        let old_target = school_skills.join("typescript").join("coding");
        let new_target = school_skills.join("typescript").join("coding-v2");
        std::fs::create_dir_all(&old_target).expect("old target");
        std::fs::create_dir_all(&new_target).expect("new target");

        let project_skills = root.join("proj").join("skills");
        std::fs::create_dir_all(project_skills.join("typescript")).expect("proj nested");
        std::os::unix::fs::symlink(
            &old_target,
            project_skills.join("typescript").join("coding"),
        )
        .expect("seed managed link");

        let desired = vec![DesiredLink {
            name: "typescript/coding".to_string(),
            target: new_target.clone(),
        }];
        reconcile(&school_skills, &school_skills, &project_skills, &desired).expect("reconcile");

        let link = project_skills.join("typescript").join("coding");
        assert!(is_symlink(&link), "link should still exist");
        let resolved = std::fs::canonicalize(&link).expect("canonicalize");
        assert_eq!(
            resolved,
            std::fs::canonicalize(&new_target).expect("target")
        );
        assert!(
            project_skills.join("typescript").is_dir(),
            "parent dir should remain intact",
        );
    }

    #[test]
    #[cfg(unix)]
    fn reconcile_prunes_empty_parents_on_remove() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path();
        let school_skills = root.join("school").join("skills");
        let nested_target = school_skills.join("typescript").join("coding");
        std::fs::create_dir_all(&nested_target).expect("nested skill dir");

        let project_skills = root.join("proj").join("skills");
        std::fs::create_dir_all(project_skills.join("typescript")).expect("proj nested");
        std::os::unix::fs::symlink(
            &nested_target,
            project_skills.join("typescript").join("coding"),
        )
        .expect("seed symlink");

        // Desired set is empty: the existing managed nested link should go,
        // and the now-empty `typescript/` parent should be pruned.
        reconcile(&school_skills, &school_skills, &project_skills, &[]).expect("reconcile");

        assert!(!project_skills.join("typescript").join("coding").exists());
        assert!(
            !project_skills.join("typescript").exists(),
            "empty parent should be pruned",
        );
        assert!(project_skills.exists(), "root skills dir stays");
    }
}
