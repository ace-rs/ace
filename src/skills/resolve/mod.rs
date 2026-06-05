//! Project-side skill selection: turns `(skills, include_skills,
//! exclude_skills)` from `ace.toml` across the user / project / local scopes
//! into a per-identity verdict + provenance trace, then stamps each
//! [`Skill<Validated>`](super::Skill) into a [`Skill<Decided>`](super::Skill).
//!
//! Lives with the data it stamps (`docs/decisions/2026-06-05-resolver-dissolution.md`):
//! resolution reads and writes `Skill<S>`, so it sits *right* of `skills/` and
//! carries [`Locator`] natively — no stringly round-trip. The verdict drives
//! `ace skills` (provenance listing) and `ace explain <name>` (full chain).
//!
//! `Source` still imports leftward from `crate::resolver` until the config-merge
//! slice relocates it to `config/resolve/`.

use std::collections::BTreeMap;

use crate::config::ace_toml::AceToml;
use crate::config::tree::Tree;
use crate::skills::identity::pattern_matches;

use super::{Decided, Diagnostics, Locator, Skill, Skills, Validated};

pub use crate::resolver::Source;

/// Per-identity selection result: whether the config rules picked this skill,
/// and the ordered trace of rule applications that produced the verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Verdict {
    decision: Decision,
    trace: Vec<Entry>,
}

/// Resolution-wide outputs that don't belong to any single skill.
struct Selection {
    verdicts: BTreeMap<Locator, Verdict>,
    unknown_patterns: Vec<UnknownPattern>,
    collisions: Vec<Collision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub source: Source,
    pub field: Field,
    pub pattern: String,
    pub op: Op,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPattern {
    pub source: Source,
    pub field: Field,
    pub pattern: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collision {
    pub skill: Locator,
    pub source: Source,
}

/// Config-selection verdict — purely whether the `skills`/`include`/`exclude`
/// rules picked this skill. Name admissibility is an orthogonal axis settled at
/// `validate` (the skill never reaches resolution if inadmissible), not a
/// variant here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Included,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    SetBase,
    Added,
    Removed,
    ReAdded,
}

impl Op {
    pub fn label(self) -> &'static str {
        match self {
            Op::SetBase => "base",
            Op::Added => "added",
            Op::Removed => "removed",
            Op::ReAdded => "re-added",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Skills,
    IncludeSkills,
    ExcludeSkills,
}

impl Field {
    pub fn label(self) -> &'static str {
        match self {
            Field::Skills => "skills",
            Field::IncludeSkills => "include_skills",
            Field::ExcludeSkills => "exclude_skills",
        }
    }
}

impl Skills<Validated> {
    /// Run the three-layer selection against the given config tree. Consumes
    /// `self` — the typestate transition is one-way. Each validated skill is
    /// stamped with its verdict + trace; rejects were already partitioned out
    /// by `validate`, so selection runs over the admissible set only.
    pub fn resolve(self, tree: &Tree) -> Skills<Decided> {
        let default = AceToml::default();
        let user = tree.user.as_ref().unwrap_or(&default);
        let project = tree.project.as_ref().unwrap_or(&default);
        let local = tree.local.as_ref().unwrap_or(&default);

        let locators: Vec<Locator> = self.items.iter().map(|s| s.locator.clone()).collect();
        let mut selection = select(&locators, user, project, local);

        // Stamp each owned skill with its own verdict — keyed by the `Locator`
        // it already carries, so there is no name round-trip to rejoin. Every
        // validated locator was seeded into `select`, so the lookup never misses.
        let mut items: Vec<Skill<Decided>> = self
            .items
            .into_iter()
            .map(|s| {
                let Verdict { decision, trace } = selection
                    .verdicts
                    .remove(&s.locator)
                    .expect("every validated locator is seeded in select");
                s.with_state(Decided { decision, trace })
            })
            .collect();
        items.sort_by(|a, b| a.locator.cmp(&b.locator));

        Skills {
            items,
            diagnostics: Diagnostics {
                unknown_patterns: selection.unknown_patterns,
                collisions: selection.collisions,
            },
            rejected: Vec::new(),
        }
    }
}

/// Pure selection core: seed every locator as excluded, apply the base filter,
/// then the exclude and include phases, recording a trace entry per rule that
/// touches a skill. Identity-only — it never sees the discovery payload.
fn select(locators: &[Locator], user: &AceToml, project: &AceToml, local: &AceToml) -> Selection {
    let mut state: BTreeMap<Locator, Verdict> = locators
        .iter()
        .map(|loc| {
            (
                loc.clone(),
                Verdict {
                    decision: Decision::Excluded,
                    trace: Vec::new(),
                },
            )
        })
        .collect();
    let mut unknown_patterns: Vec<UnknownPattern> = Vec::new();

    apply_base(&mut state, &mut unknown_patterns, user, project, local);
    apply_phase(
        &mut state,
        &mut unknown_patterns,
        Phase::Exclude,
        scoped(user, project, local, |a| &a.exclude_skills),
    );
    apply_phase(
        &mut state,
        &mut unknown_patterns,
        Phase::Include,
        scoped(user, project, local, |a| &a.include_skills),
    );

    let collisions = detect_collisions(&state);

    Selection {
        verdicts: state,
        unknown_patterns,
        collisions,
    }
}

fn scoped<'a, F>(
    user: &'a AceToml,
    project: &'a AceToml,
    local: &'a AceToml,
    pick: F,
) -> Vec<(Source, &'a [String])>
where
    F: Fn(&'a AceToml) -> &'a Vec<String>,
{
    vec![
        (Source::User, pick(user).as_slice()),
        (Source::Project, pick(project).as_slice()),
        (Source::Local, pick(local).as_slice()),
    ]
}

fn apply_base(
    state: &mut BTreeMap<Locator, Verdict>,
    unknown: &mut Vec<UnknownPattern>,
    user: &AceToml,
    project: &AceToml,
    local: &AceToml,
) {
    let winner = if !local.skills.is_empty() {
        Some((Source::Local, &local.skills))
    } else if !project.skills.is_empty() {
        Some((Source::Project, &project.skills))
    } else if !user.skills.is_empty() {
        Some((Source::User, &user.skills))
    } else {
        None
    };

    let Some((source, patterns)) = winner else {
        for verdict in state.values_mut() {
            verdict.trace.push(Entry {
                source: Source::Default,
                field: Field::Skills,
                pattern: "*".to_string(),
                op: Op::SetBase,
            });
            verdict.decision = Decision::Included;
        }
        return;
    };

    for pattern in patterns {
        let mut matched = false;
        for (locator, verdict) in state.iter_mut() {
            if !pattern_matches(pattern, locator.as_str()) {
                continue;
            }
            matched = true;
            verdict.trace.push(Entry {
                source,
                field: Field::Skills,
                pattern: pattern.clone(),
                op: Op::SetBase,
            });
            verdict.decision = Decision::Included;
        }
        if !matched {
            unknown.push(UnknownPattern {
                source,
                field: Field::Skills,
                pattern: pattern.clone(),
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Exclude,
    Include,
}

impl Phase {
    fn field(self) -> Field {
        match self {
            Phase::Exclude => Field::ExcludeSkills,
            Phase::Include => Field::IncludeSkills,
        }
    }

    fn decision(self) -> Decision {
        match self {
            Phase::Exclude => Decision::Excluded,
            Phase::Include => Decision::Included,
        }
    }

    fn op_for(self, verdict: &Verdict) -> Option<Op> {
        match (self, verdict.decision) {
            (Phase::Exclude, Decision::Excluded) => None,
            (Phase::Exclude, Decision::Included) => Some(Op::Removed),
            (Phase::Include, Decision::Included) => Some(Op::Added),
            (Phase::Include, Decision::Excluded) => {
                let was_removed = verdict.trace.iter().any(|e| e.op == Op::Removed);
                Some(if was_removed { Op::ReAdded } else { Op::Added })
            }
        }
    }
}

fn apply_phase(
    state: &mut BTreeMap<Locator, Verdict>,
    unknown: &mut Vec<UnknownPattern>,
    phase: Phase,
    sources: Vec<(Source, &[String])>,
) {
    let field = phase.field();
    for (source, patterns) in sources {
        for pattern in patterns {
            let mut matched = false;
            for (locator, verdict) in state.iter_mut() {
                if !pattern_matches(pattern, locator.as_str()) {
                    continue;
                }
                matched = true;
                let Some(op) = phase.op_for(verdict) else {
                    continue;
                };
                verdict.trace.push(Entry {
                    source,
                    field,
                    pattern: pattern.clone(),
                    op,
                });
                verdict.decision = phase.decision();
            }
            if !matched {
                unknown.push(UnknownPattern {
                    source,
                    field,
                    pattern: pattern.clone(),
                });
            }
        }
    }
}

fn detect_collisions(state: &BTreeMap<Locator, Verdict>) -> Vec<Collision> {
    let mut collisions = Vec::new();
    for (locator, verdict) in state {
        for target in [Source::User, Source::Project, Source::Local] {
            let has_remove = verdict
                .trace
                .iter()
                .any(|e| e.source == target && e.field == Field::ExcludeSkills);
            let has_add = verdict
                .trace
                .iter()
                .any(|e| e.source == target && e.field == Field::IncludeSkills);
            if has_remove && has_add {
                collisions.push(Collision {
                    skill: locator.clone(),
                    source: target,
                });
            }
        }
    }
    collisions
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ace(skills: &[&str], include: &[&str], exclude: &[&str]) -> AceToml {
        AceToml {
            skills: skills.iter().map(|s| s.to_string()).collect(),
            include_skills: include.iter().map(|s| s.to_string()).collect(),
            exclude_skills: exclude.iter().map(|s| s.to_string()).collect(),
            ..AceToml::default()
        }
    }

    fn locators(names: &[&str]) -> Vec<Locator> {
        names.iter().map(|s| Locator::from_basename(*s)).collect()
    }

    fn names() -> Vec<Locator> {
        locators(&["a", "b", "rust-coding", "rust-fmt", "issue-tracker"])
    }

    fn included(s: &Selection) -> Vec<&str> {
        let mut v: Vec<&str> = s
            .verdicts
            .iter()
            .filter(|(_, ver)| matches!(ver.decision, Decision::Included))
            .map(|(loc, _)| loc.as_str())
            .collect();
        v.sort();
        v
    }

    fn excluded(s: &Selection) -> Vec<&str> {
        let mut v: Vec<&str> = s
            .verdicts
            .iter()
            .filter(|(_, ver)| matches!(ver.decision, Decision::Excluded))
            .map(|(loc, _)| loc.as_str())
            .collect();
        v.sort();
        v
    }

    fn verdict<'a>(s: &'a Selection, name: &str) -> &'a Verdict {
        s.verdicts
            .get(name)
            .unwrap_or_else(|| panic!("skill {name} missing from selection"))
    }

    #[test]
    fn all_empty_includes_everything_with_default_base() {
        let s = select(
            &names(),
            &AceToml::default(),
            &AceToml::default(),
            &AceToml::default(),
        );
        assert_eq!(
            included(&s),
            vec!["a", "b", "issue-tracker", "rust-coding", "rust-fmt"]
        );
        assert!(excluded(&s).is_empty());
        let v = verdict(&s, "a");
        assert_eq!(v.trace.len(), 1);
        assert_eq!(v.trace[0].source, Source::Default);
        assert_eq!(v.trace[0].field, Field::Skills);
        assert_eq!(v.trace[0].op, Op::SetBase);
    }

    #[test]
    fn project_skills_filter_narrows_base() {
        let s = select(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&s), vec!["rust-coding", "rust-fmt"]);
        let rc = verdict(&s, "rust-coding");
        assert_eq!(rc.trace[0].source, Source::Project);
        assert_eq!(rc.trace[0].pattern, "rust-*");
        assert_eq!(rc.trace[0].op, Op::SetBase);
        let a = verdict(&s, "a");
        assert!(a.trace.is_empty());
        assert_eq!(a.decision, Decision::Excluded);
    }

    #[test]
    fn local_skills_overrides_project_skills() {
        let s = select(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &ace(&["a"], &[], &[]),
        );
        assert_eq!(included(&s), vec!["a"]);
        let a = verdict(&s, "a");
        assert_eq!(a.trace[0].source, Source::Local);
    }

    #[test]
    fn user_include_skills_adds_to_project_base() {
        let s = select(
            &names(),
            &ace(&[], &["issue-*"], &[]),
            &ace(&["rust-*"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(
            included(&s),
            vec!["issue-tracker", "rust-coding", "rust-fmt"]
        );
        let it = verdict(&s, "issue-tracker");
        assert_eq!(it.trace.len(), 1);
        assert_eq!(it.trace[0].source, Source::User);
        assert_eq!(it.trace[0].field, Field::IncludeSkills);
        assert_eq!(it.trace[0].op, Op::Added);
    }

    #[test]
    fn local_exclude_skills_removes_from_base() {
        let s = select(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &ace(&[], &[], &["rust-fmt"]),
        );
        assert_eq!(included(&s), vec!["rust-coding"]);
        let rf = verdict(&s, "rust-fmt");
        assert_eq!(rf.decision, Decision::Excluded);
        assert_eq!(rf.trace.len(), 2);
        assert_eq!(rf.trace[0].op, Op::SetBase);
        assert_eq!(rf.trace[1].op, Op::Removed);
        assert_eq!(rf.trace[1].source, Source::Local);
        assert_eq!(rf.trace[1].field, Field::ExcludeSkills);
    }

    #[test]
    fn include_readds_excluded() {
        let s = select(
            &names(),
            &ace(&[], &["rust-fmt"], &[]),
            &ace(&["rust-*"], &[], &["rust-fmt"]),
            &AceToml::default(),
        );
        let rf = verdict(&s, "rust-fmt");
        assert_eq!(rf.decision, Decision::Included);
        assert_eq!(rf.trace.len(), 3);
        assert_eq!(rf.trace[0].op, Op::SetBase);
        assert_eq!(rf.trace[1].op, Op::Removed);
        assert_eq!(rf.trace[2].op, Op::ReAdded);
        assert_eq!(rf.trace[2].source, Source::User);
    }

    #[test]
    fn same_scope_collision_reported() {
        let s = select(
            &names(),
            &ace(&[], &["a"], &["a"]),
            &AceToml::default(),
            &AceToml::default(),
        );
        assert_eq!(s.collisions.len(), 1);
        assert_eq!(s.collisions[0].skill.as_str(), "a");
        assert_eq!(s.collisions[0].source, Source::User);
    }

    #[test]
    fn cross_scope_include_exclude_is_not_a_collision() {
        let s = select(
            &names(),
            &ace(&[], &["a"], &[]),
            &AceToml::default(),
            &ace(&[], &[], &["a"]),
        );
        assert!(s.collisions.is_empty());
    }

    #[test]
    fn unknown_pattern_surfaced() {
        let s = select(
            &names(),
            &ace(&[], &["typo-*"], &[]),
            &AceToml::default(),
            &AceToml::default(),
        );
        assert_eq!(s.unknown_patterns.len(), 1);
        assert_eq!(s.unknown_patterns[0].pattern, "typo-*");
        assert_eq!(s.unknown_patterns[0].source, Source::User);
        assert_eq!(s.unknown_patterns[0].field, Field::IncludeSkills);
    }

    #[test]
    fn glob_pattern_matches_multiple_names() {
        let s = select(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &AceToml::default(),
        );
        let rc = verdict(&s, "rust-coding");
        let rf = verdict(&s, "rust-fmt");
        assert_eq!(rc.trace[0].pattern, "rust-*");
        assert_eq!(rf.trace[0].pattern, "rust-*");
    }

    #[test]
    fn include_on_already_included_skill_adds_extra_entry() {
        let s = select(
            &names(),
            &ace(&[], &["a"], &[]),
            &ace(&["a"], &[], &[]),
            &AceToml::default(),
        );
        let a = verdict(&s, "a");
        assert_eq!(a.decision, Decision::Included);
        assert_eq!(a.trace.len(), 2);
        assert_eq!(a.trace[0].op, Op::SetBase);
        assert_eq!(a.trace[0].source, Source::Project);
        assert_eq!(a.trace[1].op, Op::Added);
        assert_eq!(a.trace[1].source, Source::User);
    }

    #[test]
    fn exact_name_pattern_matches() {
        let s = select(
            &names(),
            &AceToml::default(),
            &ace(&["rust-coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&s), vec!["rust-coding"]);
    }

    // -- bare-name leaf match (spec: selection.md § Bare names) --
    //
    // Patterns without `*` or `/` match either exactly OR the trailing
    // path segment of a nested identity. Preserves pre-nested-identity
    // UX: `--skill rust-coding` resolves regardless of whether the skill
    // lives flat or under a subpath.

    fn nested_names() -> Vec<Locator> {
        locators(&["a", "rust-coding", "typescript/coding", "python/coding"])
    }

    #[test]
    fn bare_name_matches_flat_identity_exactly() {
        let s = select(
            &nested_names(),
            &AceToml::default(),
            &ace(&["rust-coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&s), vec!["rust-coding"]);
    }

    #[test]
    fn bare_name_matches_leaf_of_nested_identity() {
        // `coding` matches identities ending in `/coding` — multi-match
        // is the intended semantics, not an ambiguity error.
        let s = select(
            &nested_names(),
            &AceToml::default(),
            &ace(&["coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&s), vec!["python/coding", "typescript/coding"]);
    }

    #[test]
    fn bare_name_no_prefix_match() {
        // `rust` should not match `rust-coding`. Only exact or leaf.
        let s = select(
            &nested_names(),
            &AceToml::default(),
            &ace(&["rust"], &[], &[]),
            &AceToml::default(),
        );
        assert!(included(&s).is_empty());
    }

    #[test]
    fn path_anchored_pattern_no_leaf_fallback() {
        // `typescript/coding` matches only that identity. `python/coding`
        // (same leaf, different path) is not included.
        let s = select(
            &nested_names(),
            &AceToml::default(),
            &ace(&["typescript/coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&s), vec!["typescript/coding"]);
    }

    #[test]
    fn glob_with_path_separator_matches_multi_segment() {
        // `*/coding` matches multi-segment identities ending in `/coding`.
        let s = select(
            &nested_names(),
            &AceToml::default(),
            &ace(&["*/coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&s), vec!["python/coding", "typescript/coding"]);
    }

    #[test]
    fn bare_name_in_exclude_drops_leaf_matches() {
        // Reuses the leaf-fallback rule on the negative side: bare-name
        // `coding` drops `python/coding` and `typescript/coding` (leaf
        // == `coding`) but NOT `rust-coding` (leaf is `rust-coding`,
        // distinct from `coding`).
        let s = select(
            &nested_names(),
            &AceToml::default(),
            &AceToml::default(),
            &ace(&[], &[], &["coding"]),
        );
        assert_eq!(included(&s), vec!["a", "rust-coding"]);
    }
}
