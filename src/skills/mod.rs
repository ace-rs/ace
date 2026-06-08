//! Unified skills domain: typestate over discovery → validation → resolution.
//!
//! `Skill<S>` carries locator/path/tier from discovery onward; the marker `S`
//! advances `Discovered → Validated → Decided`. `validate` partitions the
//! discovered set on name admissibility (admissible skills advance, the rest
//! split off as [`Rejected`]); `resolve` then runs selection over the
//! validated set, and the `Decided` marker adds the verdict + provenance
//! trace. `Skills<S>` is the collection plus its resolution-wide diagnostics
//! (unknown patterns + collisions) and the carried reject list.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

pub mod discover;
pub mod identity;
pub mod name;
pub mod resolve;

// The skills module's identity type — carried on every `Skill<S>` from
// discovery through resolution.
pub use identity::Locator;

use crate::config::ConfigError;
use crate::school::SchoolError;

use discover::Tier;

pub use crate::config::resolve::Source;
pub use resolve::{Collision, Decision, Entry, InvalidPattern, UnknownPattern};

/// Errors that can occur while building the resolved SkillSet. Wraps
/// upstream binding errors plus skill-specific I/O failures.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error(transparent)]
    TreeLoad(#[from] ConfigError),
    #[error(transparent)]
    School(#[from] SchoolError),
    #[error("skill discovery failed: {0}")]
    Discovery(#[from] std::io::Error),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Modified,
    Removed,
}

impl ChangeKind {
    /// Single-character prefix used in pull-summary output.
    /// `+` added, `~` modified, `-` removed.
    pub fn prefix(&self) -> char {
        match self {
            ChangeKind::Added => '+',
            ChangeKind::Modified => '~',
            ChangeKind::Removed => '-',
        }
    }
}

#[derive(Debug)]
pub struct SkillChange {
    pub name: String,
    pub kind: ChangeKind,
}

/// Render a pull summary. Both `ace pull` and `ace school pull` emit through
/// this helper so the user-visible shape stays identical:
///
/// ```text
/// School updated
///   +new-skill
///   ~existing
///   -old-skill
/// ```
///
/// Empty input collapses to a single `School updated (no skill changes)`
/// line. The caller passes the result to `ace.done()`.
pub fn format_pull_summary(changes: &[SkillChange]) -> String {
    if changes.is_empty() {
        return "School updated (no skill changes)".to_string();
    }
    let mut msg = String::from("School updated");
    for change in changes {
        msg.push_str(&format!("\n  {}{}", change.kind.prefix(), change.name));
    }
    msg
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Discovered;

/// Marker proving `validate` ran in this process — the skill's identity
/// passed the admission gate. Carries no payload (the proof is set
/// membership) and persists nothing; rebuilt from scratch every run.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Validated;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decided {
    pub decision: Decision,
    pub trace: Vec<Entry>,
}

/// A skill dropped by `validate` because its identity is inadmissible.
/// The partition's second output — carries the identity and reason so
/// warnings and the `ace skills` listing can surface it, kept out of the
/// admissible set so downstream boundaries can trust the type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejected {
    pub locator: Locator,
    pub tier: Tier,
    pub reason: name::RejectReason,
    /// Origin label (`owner/repo`) when the rejected skill came from an import
    /// source, carried over from discovery. `None` for school-local skills.
    /// Lets the pull reject warning name which source shipped the bad skill.
    pub source: Option<String>,
}

mod sealed {
    pub trait Sealed {}
}

/// Persist gate: a skill whose identity already passed the admission partition.
/// Sealed, implemented by [`Validated`] and [`Decided`] but never [`Discovered`],
/// so write-to-disk boundaries (`copy_into`) are *unrepresentable* for a skill
/// that has not been validated — "validate before you persist" is compiler-enforced.
pub trait Vetted: sealed::Sealed {}

impl sealed::Sealed for Validated {}
impl sealed::Sealed for Decided {}
impl Vetted for Validated {}
impl Vetted for Decided {}

#[derive(Debug, Clone)]
pub struct Skill<S> {
    /// Path-shaped identity, minted by discovery and carried unchanged
    /// through resolution and emit. The single source of truth for which
    /// skill this is.
    pub locator: Locator,
    pub path: PathBuf,
    pub tier: Tier,
    /// `internal: true` in SKILL.md frontmatter. Used by the imports
    /// resolver to gate skills behind explicit-name matches or the
    /// `include_internal` flag (mirrors skills.sh).
    pub internal: bool,
    /// Frontmatter `name:` value, when present. Display/diagnostic only —
    /// the emit name is `basename(identity)`, never this field, and ACE never
    /// matches on it (`docs/decisions/2026-06-01-skill-name-is-path.md`). The
    /// imports resolver reads it to flag cross-source `name` mismatches.
    pub frontmatter_name: Option<String>,
    /// Origin label (`owner/repo`) when the skill was pulled from an import
    /// source. `None` for skills discovered directly from a school's own
    /// `skills/` tree.
    pub source: Option<String>,
    pub state: S,
}

impl<S> Skill<S> {
    /// Advance the atom to a new lifecycle state, preserving every intrinsic
    /// field. The marker is the only thing that changes — used by `validate`
    /// to move an admitted skill from `Discovered` to `Validated`.
    fn with_state<T>(self, state: T) -> Skill<T> {
        Skill {
            locator: self.locator,
            path: self.path,
            tier: self.tier,
            internal: self.internal,
            frontmatter_name: self.frontmatter_name,
            source: self.source,
            state,
        }
    }
}

impl Skill<Discovered> {
    /// Character + structural admissibility of this skill's identity path.
    /// Recomputed at `validate` (the admission gate of record, model.md
    /// § Name Admission) rather than carried — a pure function of identity,
    /// so it never goes stale. Orthogonal to the resolver's selection
    /// [`Decision`].
    pub fn admission(&self) -> Result<(), name::RejectReason> {
        name::admissible_skill(self.locator.as_str())
    }

    /// Display-hygiene warning for an admitted skill whose frontmatter `name:`
    /// carries spoofable characters (bidi/control) or a non-token shape. The
    /// skill is *not* rejected — frontmatter is display-only, never emitted or
    /// matched ([`admission`](Self::admission) ignores it) — but the author who
    /// imported it should know. `None` when the name is absent or clean.
    /// Surfaced only at authoring boundaries (`ace import`, `ace school pull`).
    pub fn frontmatter_warning(&self) -> Option<String> {
        let name = self.frontmatter_name.as_deref()?;
        let reason = name::admissible_component(name, name::NameContext::FrontmatterName).err()?;
        Some(format!(
            "skill `{}` admitted with an unsafe frontmatter `name:` — {reason}",
            name::render(self.locator.as_str()),
        ))
    }
}

/// Hint paired with [`Skill::frontmatter_warning`]. The name is display-only,
/// so the fix is upstream or selection-side, never an ACE edit.
pub const FRONTMATTER_WARNING_HINT: &str = "ACE emits the path basename and renders the name sanitized, but the backend reads \
     the frontmatter raw — verify the source or drop the skill via `exclude_skills`";

/// Selection verdict of a *decided* skill. A `Skill<Decided>` is admissible
/// by lineage (it came through `validate`), so rejection is no longer a
/// status here — inadmissible skills are partitioned off as [`Rejected`]
/// before resolution and never reach this type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Status {
    Active,
    Excluded,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Active => "active",
            Status::Excluded => "excluded",
        }
    }
}

impl Skill<Decided> {
    pub fn status(&self) -> Status {
        match self.state.decision {
            Decision::Included => Status::Active,
            Decision::Excluded => Status::Excluded,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Skills<S> {
    items: Vec<Skill<S>>,
    diagnostics: Diagnostics,
    /// Skills `validate` dropped as inadmissible. Populated on the resolved
    /// collection (via [`Skills::with_rejected`]); empty at earlier stages.
    /// Carried alongside the admissible set so warnings and the listing can
    /// surface rejections without keeping them in `items`.
    rejected: Vec<Rejected>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub unknown_patterns: Vec<UnknownPattern>,
    pub invalid_patterns: Vec<InvalidPattern>,
    pub collisions: Vec<Collision>,
}

// ---- Skills<Discovered> ----

impl Skills<Discovered> {
    /// Walk the school's `skills/` tree. See `discover::discover_skills` for
    /// the tier priority order. Returns the discovered set plus any structural
    /// prunes (malformed identities) for the caller to surface — see
    /// `discover_skills`.
    pub fn discover(school_root: &Path) -> io::Result<(Self, Vec<name::RejectReason>)> {
        let (skills, prunes) = discover::discover_skills(school_root)?;
        Ok((Self::from_discovered(&skills), prunes))
    }

    pub fn from_discovered(discovered: &[Skill<Discovered>]) -> Self {
        Self::from_skills(discovered.to_vec())
    }

    /// Wrap an owned set of discovered skills as-is, preserving each skill's
    /// `source` label. Used by `pull_imports` after mapping resolved imports
    /// back to their source-tagged discovery records.
    pub fn from_skills(items: Vec<Skill<Discovered>>) -> Self {
        Self {
            items,
            diagnostics: Diagnostics::default(),
            rejected: Vec::new(),
        }
    }

    /// Partition the discovered set on name admissibility: admissible skills
    /// advance to `Skills<Validated>`, inadmissible ones split off as
    /// [`Rejected`]. This is the admission gate — `validate` *removes* rejects
    /// rather than tagging them, so every later stage can trust that its skills
    /// are admissible by construction. Re-runs from scratch each process, so it
    /// self-heals when the admission rules tighten.
    pub fn validate(self) -> (Skills<Validated>, Vec<Rejected>) {
        let mut admitted = Vec::new();
        let mut rejected = Vec::new();
        for skill in self.items {
            match skill.admission() {
                Ok(()) => admitted.push(skill.with_state(Validated)),
                Err(reason) => rejected.push(Rejected {
                    locator: skill.locator,
                    tier: skill.tier,
                    reason,
                    source: skill.source,
                }),
            }
        }
        let validated = Skills {
            items: admitted,
            diagnostics: self.diagnostics,
            rejected: Vec::new(),
        };
        (validated, rejected)
    }
}

// ---- Skills<S> (stage-agnostic) ----

impl<S> Skills<S> {
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(|s| s.locator.as_str())
    }
}

// ---- Skills<S: Vetted> (persist boundary) ----

impl<S: Vetted> Skills<S> {
    /// Copy named skills into `dest_dir`. Each skill is classified Added
    /// (didn't exist) or Modified (overwrote). Unknown names silently skipped.
    /// Gated on [`Vetted`]: only a validated-or-later set can be written to
    /// disk — an unadmitted identity is unrepresentable here.
    pub fn copy_into(&self, dest_dir: &Path, names: &[&str]) -> io::Result<Vec<SkillChange>> {
        let by_name: HashMap<&str, &Skill<S>> =
            self.items.iter().map(|s| (s.locator.as_str(), s)).collect();

        let mut changes = Vec::new();
        for &name in names {
            let Some(skill) = by_name.get(name) else {
                continue;
            };

            let dest = dest_dir.join(name);
            let kind = if dest.exists() {
                std::fs::remove_dir_all(&dest)?;
                ChangeKind::Modified
            } else {
                ChangeKind::Added
            };

            crate::fsutil::copy_dir_recursive(&skill.path, &dest)?;
            changes.push(SkillChange {
                name: name.to_string(),
                kind,
            });
        }
        Ok(changes)
    }
}

// ---- Skills<Decided> ----

impl Skills<Decided> {
    pub fn find(&self, name: &str) -> Option<&Skill<Decided>> {
        self.items.iter().find(|s| s.locator.as_str() == name)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Skill<Decided>> {
        self.items.iter()
    }

    /// Skills the resolver selected. All are admissible by lineage — rejects
    /// were partitioned off at `validate`, see [`Self::rejected`].
    pub fn included(&self) -> impl Iterator<Item = &Skill<Decided>> {
        self.with_status(Status::Active)
    }

    /// Skills that exist in the school but were filtered out by the resolved
    /// `include_skills` / `exclude_skills` rules.
    pub fn excluded(&self) -> impl Iterator<Item = &Skill<Decided>> {
        self.with_status(Status::Excluded)
    }

    fn with_status(&self, status: Status) -> impl Iterator<Item = &Skill<Decided>> {
        self.items.iter().filter(move |s| s.status() == status)
    }

    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }

    /// Skills `validate` dropped as inadmissible. Attached by [`Self::with_rejected`]
    /// after resolution; warnings and the `ace skills` listing surface them.
    pub fn rejected(&self) -> &[Rejected] {
        &self.rejected
    }

    /// Attach the reject list from `validate` onto the resolved collection.
    /// Called once, right after `resolve`, to reunite the partition's two
    /// halves for the consumers that report both.
    pub fn with_rejected(mut self, rejected: Vec<Rejected>) -> Self {
        self.rejected = rejected;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ace_toml::AceToml;
    use crate::config::tree::Tree;

    #[test]
    fn pull_summary_empty() {
        let summary = format_pull_summary(&[]);
        assert_eq!(summary, "School updated (no skill changes)");
    }

    #[test]
    fn pull_summary_with_changes() {
        let changes = [
            SkillChange {
                name: "added".to_string(),
                kind: ChangeKind::Added,
            },
            SkillChange {
                name: "edit".to_string(),
                kind: ChangeKind::Modified,
            },
            SkillChange {
                name: "gone".to_string(),
                kind: ChangeKind::Removed,
            },
        ];
        let summary = format_pull_summary(&changes);
        assert_eq!(summary, "School updated\n  +added\n  ~edit\n  -gone",);
    }

    fn ace(skills: &[&str], inc: &[&str], exc: &[&str]) -> AceToml {
        AceToml {
            skills: skills.iter().map(|s| s.to_string()).collect(),
            include_skills: inc.iter().map(|s| s.to_string()).collect(),
            exclude_skills: exc.iter().map(|s| s.to_string()).collect(),
            ..AceToml::default()
        }
    }

    fn tree(project: AceToml) -> Tree {
        Tree {
            user: None,
            project: Some(project),
            local: None,
            school: None,
        }
    }

    fn discovered(name: &str, tier: Tier) -> Skill<Discovered> {
        Skill {
            locator: Locator::from_basename(name),
            path: PathBuf::from(format!("/school/{name}")),
            tier,
            internal: false,
            frontmatter_name: None,
            source: None,
            state: Discovered,
        }
    }

    #[test]
    fn resolve_preserves_path_and_tier() {
        let s = Skills::<Discovered>::from_discovered(&[
            discovered("a", Tier::Curated),
            discovered("b", Tier::Experimental),
        ]);
        let resolved = s.validate().0.resolve(&tree(AceToml::default()));

        let a = resolved.find("a").expect("a");
        assert_eq!(a.path, PathBuf::from("/school/a"));
        assert_eq!(a.tier, Tier::Curated);
        assert_eq!(a.state.decision, Decision::Included); // implicit base
        assert_eq!(a.state.trace.len(), 1);

        let b = resolved.find("b").expect("b");
        assert_eq!(b.tier, Tier::Experimental);
    }

    #[test]
    fn resolve_output_sorted_by_locator() {
        // Discovery order is arbitrary; resolution sorts by identity so the
        // `ace skills` listing is stable regardless of on-disk walk order.
        let s = Skills::<Discovered>::from_discovered(&[
            discovered("c", Tier::Curated),
            discovered("a", Tier::Curated),
            discovered("b", Tier::Curated),
        ]);
        let resolved = s.validate().0.resolve(&tree(AceToml::default()));
        let order: Vec<&str> = resolved.iter().map(|s| s.locator.as_str()).collect();
        assert_eq!(order, vec!["a", "b", "c"]);
    }

    #[test]
    fn validate_partitions_out_inadmissible_identity() {
        let (validated, rejected) = Skills::<Discovered>::from_discovered(&[
            discovered("safe", Tier::Curated),
            discovered("bad\u{202E}name", Tier::Curated),
        ])
        .validate();

        // The bad skill split off as a Rejected; the clean one advanced.
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].locator, "bad\u{202E}name");

        let resolved = validated
            .resolve(&tree(AceToml::default()))
            .with_rejected(rejected);
        let included: Vec<&str> = resolved.included().map(|s| s.locator.as_str()).collect();
        assert_eq!(included, vec!["safe"]);
        assert_eq!(resolved.rejected().len(), 1);
        assert_eq!(resolved.rejected()[0].locator, "bad\u{202E}name");
    }

    #[test]
    fn validate_carries_source_onto_rejected() {
        // The rejected half keeps the skill's origin so pull's multi-source
        // reject warning can still name which source shipped the bad skill.
        let mut skill = discovered("bad\u{202E}name", Tier::Curated);
        skill.source = Some("owner/repo".to_string());
        let (_validated, rejected) = Skills::from_skills(vec![skill]).validate();

        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].source.as_deref(), Some("owner/repo"));
    }

    #[test]
    fn frontmatter_name_does_not_gate_admission() {
        // Identity path is clean; frontmatter `name:` is the backend's domain
        // and never rejects the skill (name = basename(identity)).
        let mut skill = discovered("safe", Tier::Curated);
        skill.frontmatter_name = Some("bad\u{202E}name".to_string());
        let (validated, rejected) = Skills::<Discovered>::from_discovered(&[skill]).validate();

        assert!(rejected.is_empty());
        let resolved = validated.resolve(&tree(AceToml::default()));
        assert_eq!(resolved.included().count(), 1);
    }

    fn excluded_names(resolved: &Skills<Decided>) -> Vec<String> {
        let mut names: Vec<String> = resolved.excluded().map(|s| s.locator.to_string()).collect();
        names.sort();
        names
    }

    #[test]
    fn excluded_empty_when_no_filters() {
        let s = Skills::<Discovered>::from_discovered(&[
            discovered("a", Tier::Curated),
            discovered("b", Tier::Curated),
        ]);
        let resolved = s.validate().0.resolve(&tree(AceToml::default()));
        assert!(excluded_names(&resolved).is_empty());
    }

    #[test]
    fn excluded_returns_filtered_names_include_only() {
        let s = Skills::<Discovered>::from_discovered(&[
            discovered("a", Tier::Curated),
            discovered("b", Tier::Curated),
            discovered("c", Tier::Curated),
        ]);
        let resolved = s.validate().0.resolve(&tree(ace(&["a"], &[], &[])));
        // include_skills via `skills = ["a"]` narrows the active set.
        assert_eq!(excluded_names(&resolved), vec!["b", "c"]);
    }

    #[test]
    fn excluded_returns_filtered_names_exclude_only() {
        let s = Skills::<Discovered>::from_discovered(&[
            discovered("a", Tier::Curated),
            discovered("b", Tier::Curated),
        ]);
        let resolved = s.validate().0.resolve(&tree(ace(&[], &[], &["a"])));
        assert_eq!(excluded_names(&resolved), vec!["a"]);
    }

    #[test]
    fn excluded_returns_filtered_names_include_and_exclude() {
        let s = Skills::<Discovered>::from_discovered(&[
            discovered("a", Tier::Curated),
            discovered("b", Tier::Curated),
            discovered("c", Tier::Curated),
        ]);
        let resolved = s.validate().0.resolve(&tree(ace(&["a", "b"], &[], &["b"])));
        assert_eq!(excluded_names(&resolved), vec!["b", "c"]);
    }

    #[test]
    fn included_filters_excluded() {
        let s = Skills::<Discovered>::from_discovered(&[
            discovered("a", Tier::Curated),
            discovered("b", Tier::Curated),
        ]);
        let resolved = s.validate().0.resolve(&tree(ace(&["a"], &[], &[])));

        let included: Vec<&str> = resolved.included().map(|s| s.locator.as_str()).collect();
        assert_eq!(included, vec!["a"]);

        // Both still iterable; only `b` is excluded.
        assert_eq!(resolved.iter().count(), 2);
    }

    #[test]
    fn diagnostics_carry_unknown_patterns() {
        let s = Skills::<Discovered>::from_discovered(&[discovered("a", Tier::Curated)]);
        let resolved = s
            .validate()
            .0
            .resolve(&tree(ace(&["nonexistent"], &[], &[])));

        let unk = &resolved.diagnostics().unknown_patterns;
        assert_eq!(unk.len(), 1);
        assert_eq!(unk[0].pattern, "nonexistent");
    }

    #[test]
    fn copy_into_adds_and_modifies() {
        use std::fs;
        let src = tempfile::tempdir().expect("src");
        let dest = tempfile::tempdir().expect("dest");

        // Stage one source skill on disk so copy_dir_recursive has something
        // to copy.
        let skill_dir = src.path().join("my-skill");
        fs::create_dir_all(&skill_dir).expect("mkdir");
        fs::write(skill_dir.join("SKILL.md"), "# my-skill").expect("write");

        let s = Skills::<Discovered>::from_discovered(&[Skill {
            locator: Locator::from_basename("my-skill"),
            path: skill_dir,
            tier: Tier::Curated,
            internal: false,
            frontmatter_name: None,
            source: None,
            state: Discovered,
        }])
        .validate()
        .0;

        let added = s.copy_into(dest.path(), &["my-skill"]).expect("copy");
        assert_eq!(added.len(), 1);
        assert_eq!(added[0].kind, ChangeKind::Added);
        assert!(dest.path().join("my-skill/SKILL.md").exists());

        let modified = s.copy_into(dest.path(), &["my-skill"]).expect("copy");
        assert_eq!(modified[0].kind, ChangeKind::Modified);
    }

    #[test]
    fn copy_into_skips_unknown() {
        let dest = tempfile::tempdir().expect("dest");
        let s = Skills::<Discovered>::from_discovered(&[]).validate().0;
        let changes = s.copy_into(dest.path(), &["nonexistent"]).expect("copy");
        assert!(changes.is_empty());
    }

    #[test]
    fn from_skills_preserves_source_label() {
        let mut skill = discovered("alpha", Tier::Curated);
        skill.source = Some("owner/a".to_string());
        let s = Skills::from_skills(vec![skill]);
        assert_eq!(s.items[0].source.as_deref(), Some("owner/a"));
    }
}
