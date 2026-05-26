//! Project-side skill resolution: turns `(skills, include_skills,
//! exclude_skills)` from `ace.toml` across the user / project / local
//! scopes into a structured trace per discovered identity.
//!
//! Pure logic. The trace drives `ace skills` (provenance listing) and
//! `ace explain <name>` (full chain). Today's `Scope::Implicit` is folded
//! into the unified `Source::Default`.
//!
//! Sibling to the imports resolver (added in a later slice), which merges
//! `[[imports]]` declarations within a single `school.toml`. They share
//! glob + trace primitives but diverge on scope taxonomy and verdict
//! variants — see `docs/spec/skills/selection.md` § Provenance.

use std::collections::BTreeMap;

use crate::config::ace_toml::AceToml;
use crate::skills::identity::pattern_matches;

use super::source::Source;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    pub skills: Vec<ResolvedSkill>,
    pub unknown_patterns: Vec<UnknownPattern>,
    pub collisions: Vec<Collision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedSkill {
    pub name: String,
    pub decision: Decision,
    pub trace: Vec<Entry>,
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
    pub skill: String,
    pub source: Source,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Included,
    Excluded,
}

impl Decision {
    pub fn label(self) -> &'static str {
        match self {
            Decision::Included => "active",
            Decision::Excluded => "excluded",
        }
    }
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

pub fn resolve_skills(
    discovered: &[String],
    user: &AceToml,
    project: &AceToml,
    local: &AceToml,
) -> Resolution {
    let mut state: BTreeMap<String, ResolvedSkill> = discovered
        .iter()
        .map(|name| {
            (
                name.clone(),
                ResolvedSkill {
                    name: name.clone(),
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

    Resolution {
        skills: state.into_values().collect(),
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
    state: &mut BTreeMap<String, ResolvedSkill>,
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
        for skill in state.values_mut() {
            skill.trace.push(Entry {
                source: Source::Default,
                field: Field::Skills,
                pattern: "*".to_string(),
                op: Op::SetBase,
            });
            skill.decision = Decision::Included;
        }
        return;
    };

    for pattern in patterns {
        let mut matched = false;
        for skill in state.values_mut() {
            if !pattern_matches(pattern, &skill.name) {
                continue;
            }
            matched = true;
            skill.trace.push(Entry {
                source,
                field: Field::Skills,
                pattern: pattern.clone(),
                op: Op::SetBase,
            });
            skill.decision = Decision::Included;
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

    fn op_for(self, skill: &ResolvedSkill) -> Option<Op> {
        match (self, skill.decision) {
            (Phase::Exclude, Decision::Excluded) => None,
            (Phase::Exclude, Decision::Included) => Some(Op::Removed),
            (Phase::Include, Decision::Included) => Some(Op::Added),
            (Phase::Include, Decision::Excluded) => {
                let was_removed = skill.trace.iter().any(|e| e.op == Op::Removed);
                Some(if was_removed { Op::ReAdded } else { Op::Added })
            }
        }
    }
}

fn apply_phase(
    state: &mut BTreeMap<String, ResolvedSkill>,
    unknown: &mut Vec<UnknownPattern>,
    phase: Phase,
    sources: Vec<(Source, &[String])>,
) {
    let field = phase.field();
    for (source, patterns) in sources {
        for pattern in patterns {
            let mut matched = false;
            for skill in state.values_mut() {
                if !pattern_matches(pattern, &skill.name) {
                    continue;
                }
                matched = true;
                let Some(op) = phase.op_for(skill) else {
                    continue;
                };
                skill.trace.push(Entry {
                    source,
                    field,
                    pattern: pattern.clone(),
                    op,
                });
                skill.decision = phase.decision();
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

fn detect_collisions(state: &BTreeMap<String, ResolvedSkill>) -> Vec<Collision> {
    let mut collisions = Vec::new();
    for skill in state.values() {
        for target in [Source::User, Source::Project, Source::Local] {
            let has_remove = skill
                .trace
                .iter()
                .any(|e| e.source == target && e.field == Field::ExcludeSkills);
            let has_add = skill
                .trace
                .iter()
                .any(|e| e.source == target && e.field == Field::IncludeSkills);
            if has_remove && has_add {
                collisions.push(Collision {
                    skill: skill.name.clone(),
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

    fn names() -> Vec<String> {
        ["a", "b", "rust-coding", "rust-fmt", "issue-tracker"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn included(r: &Resolution) -> Vec<&str> {
        r.skills
            .iter()
            .filter(|s| s.decision == Decision::Included)
            .map(|s| s.name.as_str())
            .collect()
    }

    fn excluded(r: &Resolution) -> Vec<&str> {
        r.skills
            .iter()
            .filter(|s| s.decision == Decision::Excluded)
            .map(|s| s.name.as_str())
            .collect()
    }

    fn find<'a>(r: &'a Resolution, name: &str) -> &'a ResolvedSkill {
        r.skills
            .iter()
            .find(|s| s.name == name)
            .unwrap_or_else(|| panic!("skill {name} missing from resolution"))
    }

    #[test]
    fn all_empty_includes_everything_with_default_base() {
        let r = resolve_skills(&names(), &AceToml::default(), &AceToml::default(), &AceToml::default());
        assert_eq!(included(&r), vec!["a", "b", "issue-tracker", "rust-coding", "rust-fmt"]);
        assert!(excluded(&r).is_empty());
        let s = find(&r, "a");
        assert_eq!(s.trace.len(), 1);
        assert_eq!(s.trace[0].source, Source::Default);
        assert_eq!(s.trace[0].field, Field::Skills);
        assert_eq!(s.trace[0].op, Op::SetBase);
    }

    #[test]
    fn project_skills_filter_narrows_base() {
        let r = resolve_skills(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&r), vec!["rust-coding", "rust-fmt"]);
        let rc = find(&r, "rust-coding");
        assert_eq!(rc.trace[0].source, Source::Project);
        assert_eq!(rc.trace[0].pattern, "rust-*");
        assert_eq!(rc.trace[0].op, Op::SetBase);
        let a = find(&r, "a");
        assert!(a.trace.is_empty());
        assert_eq!(a.decision, Decision::Excluded);
    }

    #[test]
    fn local_skills_overrides_project_skills() {
        let r = resolve_skills(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &ace(&["a"], &[], &[]),
        );
        assert_eq!(included(&r), vec!["a"]);
        let a = find(&r, "a");
        assert_eq!(a.trace[0].source, Source::Local);
    }

    #[test]
    fn user_include_skills_adds_to_project_base() {
        let r = resolve_skills(
            &names(),
            &ace(&[], &["issue-*"], &[]),
            &ace(&["rust-*"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&r), vec!["issue-tracker", "rust-coding", "rust-fmt"]);
        let it = find(&r, "issue-tracker");
        assert_eq!(it.trace.len(), 1);
        assert_eq!(it.trace[0].source, Source::User);
        assert_eq!(it.trace[0].field, Field::IncludeSkills);
        assert_eq!(it.trace[0].op, Op::Added);
    }

    #[test]
    fn local_exclude_skills_removes_from_base() {
        let r = resolve_skills(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &ace(&[], &[], &["rust-fmt"]),
        );
        assert_eq!(included(&r), vec!["rust-coding"]);
        let rf = find(&r, "rust-fmt");
        assert_eq!(rf.decision, Decision::Excluded);
        assert_eq!(rf.trace.len(), 2);
        assert_eq!(rf.trace[0].op, Op::SetBase);
        assert_eq!(rf.trace[1].op, Op::Removed);
        assert_eq!(rf.trace[1].source, Source::Local);
        assert_eq!(rf.trace[1].field, Field::ExcludeSkills);
    }

    #[test]
    fn include_readds_excluded() {
        let r = resolve_skills(
            &names(),
            &ace(&[], &["rust-fmt"], &[]),
            &ace(&["rust-*"], &[], &["rust-fmt"]),
            &AceToml::default(),
        );
        let rf = find(&r, "rust-fmt");
        assert_eq!(rf.decision, Decision::Included);
        assert_eq!(rf.trace.len(), 3);
        assert_eq!(rf.trace[0].op, Op::SetBase);
        assert_eq!(rf.trace[1].op, Op::Removed);
        assert_eq!(rf.trace[2].op, Op::ReAdded);
        assert_eq!(rf.trace[2].source, Source::User);
    }

    #[test]
    fn same_scope_collision_reported() {
        let r = resolve_skills(
            &names(),
            &ace(&[], &["a"], &["a"]),
            &AceToml::default(),
            &AceToml::default(),
        );
        assert_eq!(r.collisions.len(), 1);
        assert_eq!(r.collisions[0].skill, "a");
        assert_eq!(r.collisions[0].source, Source::User);
    }

    #[test]
    fn cross_scope_include_exclude_is_not_a_collision() {
        let r = resolve_skills(
            &names(),
            &ace(&[], &["a"], &[]),
            &AceToml::default(),
            &ace(&[], &[], &["a"]),
        );
        assert!(r.collisions.is_empty());
    }

    #[test]
    fn unknown_pattern_surfaced() {
        let r = resolve_skills(
            &names(),
            &ace(&[], &["typo-*"], &[]),
            &AceToml::default(),
            &AceToml::default(),
        );
        assert_eq!(r.unknown_patterns.len(), 1);
        assert_eq!(r.unknown_patterns[0].pattern, "typo-*");
        assert_eq!(r.unknown_patterns[0].source, Source::User);
        assert_eq!(r.unknown_patterns[0].field, Field::IncludeSkills);
    }

    #[test]
    fn glob_pattern_matches_multiple_names() {
        let r = resolve_skills(
            &names(),
            &AceToml::default(),
            &ace(&["rust-*"], &[], &[]),
            &AceToml::default(),
        );
        let rc = find(&r, "rust-coding");
        let rf = find(&r, "rust-fmt");
        assert_eq!(rc.trace[0].pattern, "rust-*");
        assert_eq!(rf.trace[0].pattern, "rust-*");
    }

    #[test]
    fn output_sorted_by_name() {
        let r = resolve_skills(&names(), &AceToml::default(), &AceToml::default(), &AceToml::default());
        let names: Vec<&str> = r.skills.iter().map(|s| s.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(names, sorted);
    }

    #[test]
    fn include_on_already_included_skill_adds_extra_entry() {
        let r = resolve_skills(
            &names(),
            &ace(&[], &["a"], &[]),
            &ace(&["a"], &[], &[]),
            &AceToml::default(),
        );
        let a = find(&r, "a");
        assert_eq!(a.decision, Decision::Included);
        assert_eq!(a.trace.len(), 2);
        assert_eq!(a.trace[0].op, Op::SetBase);
        assert_eq!(a.trace[0].source, Source::Project);
        assert_eq!(a.trace[1].op, Op::Added);
        assert_eq!(a.trace[1].source, Source::User);
    }

    #[test]
    fn exact_name_pattern_matches() {
        let r = resolve_skills(
            &names(),
            &AceToml::default(),
            &ace(&["rust-coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&r), vec!["rust-coding"]);
    }

    // -- bare-name leaf match (spec: selection.md § Bare names) --
    //
    // Patterns without `*` or `/` match either exactly OR the trailing
    // path segment of a nested identity. Preserves pre-nested-identity
    // UX: `--skill rust-coding` resolves regardless of whether the skill
    // lives flat or under a subpath.

    fn nested_names() -> Vec<String> {
        ["a", "rust-coding", "typescript/coding", "python/coding"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    #[test]
    fn bare_name_matches_flat_identity_exactly() {
        let r = resolve_skills(
            &nested_names(),
            &AceToml::default(),
            &ace(&["rust-coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&r), vec!["rust-coding"]);
    }

    #[test]
    fn bare_name_matches_leaf_of_nested_identity() {
        // `coding` matches identities ending in `/coding` — multi-match
        // is the intended semantics, not an ambiguity error.
        let r = resolve_skills(
            &nested_names(),
            &AceToml::default(),
            &ace(&["coding"], &[], &[]),
            &AceToml::default(),
        );
        let mut inc = included(&r);
        inc.sort();
        assert_eq!(inc, vec!["python/coding", "typescript/coding"]);
    }

    #[test]
    fn bare_name_no_prefix_match() {
        // `rust` should not match `rust-coding`. Only exact or leaf.
        let r = resolve_skills(
            &nested_names(),
            &AceToml::default(),
            &ace(&["rust"], &[], &[]),
            &AceToml::default(),
        );
        assert!(included(&r).is_empty());
    }

    #[test]
    fn path_anchored_pattern_no_leaf_fallback() {
        // `typescript/coding` matches only that identity. `python/coding`
        // (same leaf, different path) is not included.
        let r = resolve_skills(
            &nested_names(),
            &AceToml::default(),
            &ace(&["typescript/coding"], &[], &[]),
            &AceToml::default(),
        );
        assert_eq!(included(&r), vec!["typescript/coding"]);
    }

    #[test]
    fn glob_with_path_separator_matches_multi_segment() {
        // `*/coding` matches multi-segment identities ending in `/coding`.
        let r = resolve_skills(
            &nested_names(),
            &AceToml::default(),
            &ace(&["*/coding"], &[], &[]),
            &AceToml::default(),
        );
        let mut inc = included(&r);
        inc.sort();
        assert_eq!(inc, vec!["python/coding", "typescript/coding"]);
    }

    #[test]
    fn bare_name_in_exclude_drops_leaf_matches() {
        // Reuses the leaf-fallback rule on the negative side: bare-name
        // `coding` drops `python/coding` and `typescript/coding` (leaf
        // == `coding`) but NOT `rust-coding` (leaf is `rust-coding`,
        // distinct from `coding`).
        let r = resolve_skills(
            &nested_names(),
            &AceToml::default(),
            &AceToml::default(),
            &ace(&[], &[], &["coding"]),
        );
        let mut inc = included(&r);
        inc.sort();
        assert_eq!(inc, vec!["a", "rust-coding"]);
    }
}
