//! Imports resolver — merges `[[imports]]` declarations within a single
//! `school.toml` into a per-skill verdict with provenance.
//!
//! Sibling to the project resolver (`super::project`). Both share the
//! `pattern_matches` matcher and trace primitives, but their scope
//! taxonomies differ:
//!   - Project resolver tracks user / project / local scopes.
//!   - Imports resolver tracks `[[imports]]` declaration index + source.
//!
//! See `docs/spec/skills/selection.md` § `[[imports]]` schema, Cross-source
//! merge, and Provenance.

use std::collections::HashMap;

use crate::config::school_toml::ImportDecl;
use crate::glob;
use crate::skills::discover::Tier;
use crate::skills::identity::pattern_matches;
use crate::skills::{Discovered, Locator, Skill};

/// Resolution output: one entry per skill considered, plus diagnostics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportsResolution {
    /// Every resolved skill with its decl provenance. Included skills
    /// emit; LostTo entries identify the loser of a collision.
    pub skills: Vec<ResolvedImport>,
    /// Cross-source identity collisions surfaced to the school
    /// maintainer (and downstream consumers if not addressed).
    pub collisions: Vec<ImportCollision>,
    /// Patterns that matched nothing in their source.
    pub unknown_patterns: Vec<UnknownImportPattern>,
    /// Patterns with unsupported glob syntax — skipped, not rejected.
    pub invalid_patterns: Vec<InvalidImportPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedImport {
    pub identity: Locator,
    pub source: String,
    pub decl_index: usize,
    pub verdict: ImportVerdict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportVerdict {
    /// Will be installed into `<school>/skills/<identity>/`.
    Included,
    /// Another decl earlier in declaration order already won this
    /// identity. `winner_decl_index` points at it.
    LostTo { winner_decl_index: usize },
    /// `internal: true` skill that didn't match via explicit name and
    /// the decl didn't set `include_internal = true`.
    FilteredInternal,
    /// Tier-gated out (skill is Experimental/System but the decl flag
    /// wasn't set).
    FilteredTier,
    /// Subtracted by the decl's `exclude_skills`.
    ExcludedBySelf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportCollision {
    pub identity: Locator,
    pub winner_source: String,
    pub winner_decl_index: usize,
    pub loser_source: String,
    pub loser_decl_index: usize,
    /// True when the winning decl's `exclude_skills` covers this
    /// identity — the maintainer signalled intent, so the warning is
    /// suppressed in `surface_warnings`.
    pub suppressed_by_exclude: bool,
    /// True when the two sources' SKILL.md frontmatter `name:` differs
    /// for this identity. Promoted to its own warning surface.
    pub frontmatter_mismatch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownImportPattern {
    pub source: String,
    pub decl_index: usize,
    pub pattern: String,
}

/// An `[[imports]]` pattern whose glob syntax is unsupported (`?`,
/// `[...]`, empty). Skipped and warned at the resolver boundary — never a
/// hard error, since the school is third-party authored. `reason` is the
/// `glob::validate` message, echoed verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidImportPattern {
    pub source: String,
    pub decl_index: usize,
    pub pattern: String,
    pub reason: String,
}

/// Discovered skills across all import sources, indexed for O(1) lookup by
/// `(source, identity)`. Built incrementally as each source is fetched
/// (a source can appear in multiple decls; [`has_source`](Self::has_source)
/// lets the caller fetch each once), then consumed by [`resolve_imports`].
/// Owns its skills, so the resolution it produces stays free-standing.
#[derive(Debug, Default)]
pub struct Discovery {
    by_source: HashMap<String, SourceSkills>,
}

#[derive(Debug)]
struct SourceSkills {
    skills: Vec<Skill<Discovered>>,
    /// identity → index into `skills`, so `lookup` is O(1) rather than a scan.
    by_locator: HashMap<Locator, usize>,
}

impl Discovery {
    pub fn has_source(&self, source: &str) -> bool {
        self.by_source.contains_key(source)
    }

    /// Record one source's discovered skills, building its identity index.
    pub fn insert(&mut self, source: &str, skills: Vec<Skill<Discovered>>) {
        let by_locator = skills
            .iter()
            .enumerate()
            .map(|(i, s)| (s.locator.clone(), i))
            .collect();
        self.by_source
            .insert(source.to_string(), SourceSkills { skills, by_locator });
    }

    /// The discovered skill for `(source, identity)`, if any. O(1).
    pub fn lookup(&self, source: &str, identity: &Locator) -> Option<&Skill<Discovered>> {
        let src = self.by_source.get(source)?;
        src.by_locator.get(identity).map(|&i| &src.skills[i])
    }

    fn source_skills(&self, source: &str) -> Option<&[Skill<Discovered>]> {
        self.by_source.get(source).map(|s| s.skills.as_slice())
    }
}

/// Run the imports resolver. Walks decls in declaration order, applies
/// pattern + tier + internal filters per decl, and merges across decls
/// with first-wins-and-warn on identity collisions.
pub fn resolve_imports(decls: &[ImportDecl], discovery: &Discovery) -> ImportsResolution {
    // Stage 1: per-decl matched sets (identity → matched skill ref).
    let mut per_decl: Vec<Vec<MatchedSkill>> = Vec::with_capacity(decls.len());
    let mut unknown_patterns: Vec<UnknownImportPattern> = Vec::new();
    let mut invalid_patterns: Vec<InvalidImportPattern> = Vec::new();

    for (idx, decl) in decls.iter().enumerate() {
        let Some(discovered) = discovery.source_skills(&decl.source) else {
            // Caller is expected to provide discovery for every source
            // listed; if not, we surface every pattern as unknown.
            for pattern in decl.patterns() {
                unknown_patterns.push(UnknownImportPattern {
                    source: decl.source.clone(),
                    decl_index: idx,
                    pattern: pattern.to_string(),
                });
            }
            per_decl.push(Vec::new());
            continue;
        };
        per_decl.push(match_decl(
            decl,
            idx,
            discovered,
            &mut unknown_patterns,
            &mut invalid_patterns,
        ));
    }

    // Stage 2: cross-decl merge — first-wins on identity collision.
    // `claimed` keys identity → winning entry (and the decl that claimed it).
    let mut claimed: HashMap<Locator, ClaimEntry> = HashMap::new();
    let mut all_resolved: Vec<ResolvedImport> = Vec::new();
    let mut collisions: Vec<ImportCollision> = Vec::new();

    for (idx, matched) in per_decl.iter().enumerate() {
        for m in matched {
            // Apply per-decl filters before considering claim.
            match m.fate {
                MatchFate::Ok => {}
                MatchFate::FilteredInternal => {
                    all_resolved.push(ResolvedImport {
                        identity: m.identity.clone(),
                        source: m.source.clone(),
                        decl_index: idx,
                        verdict: ImportVerdict::FilteredInternal,
                    });
                    continue;
                }
                MatchFate::FilteredTier => {
                    all_resolved.push(ResolvedImport {
                        identity: m.identity.clone(),
                        source: m.source.clone(),
                        decl_index: idx,
                        verdict: ImportVerdict::FilteredTier,
                    });
                    continue;
                }
                MatchFate::ExcludedBySelf => {
                    all_resolved.push(ResolvedImport {
                        identity: m.identity.clone(),
                        source: m.source.clone(),
                        decl_index: idx,
                        verdict: ImportVerdict::ExcludedBySelf,
                    });
                    continue;
                }
            }

            if let Some(claim) = claimed.get(&m.identity) {
                // Loser. Surface a collision (suppressed if the winner's
                // decl excluded this identity by name/pattern).
                let suppressed = decl_excludes_identity(
                    decls.get(claim.decl_index).expect("claimed decl exists"),
                    &m.identity,
                );
                let mismatch = match (&claim.frontmatter_name, &m.frontmatter_name) {
                    (Some(a), Some(b)) => a != b,
                    _ => false,
                };
                collisions.push(ImportCollision {
                    identity: m.identity.clone(),
                    winner_source: claim.source.clone(),
                    winner_decl_index: claim.decl_index,
                    loser_source: m.source.clone(),
                    loser_decl_index: idx,
                    suppressed_by_exclude: suppressed,
                    frontmatter_mismatch: mismatch,
                });
                all_resolved.push(ResolvedImport {
                    identity: m.identity.clone(),
                    source: m.source.clone(),
                    decl_index: idx,
                    verdict: ImportVerdict::LostTo {
                        winner_decl_index: claim.decl_index,
                    },
                });
            } else {
                claimed.insert(
                    m.identity.clone(),
                    ClaimEntry {
                        decl_index: idx,
                        source: m.source.clone(),
                        frontmatter_name: m.frontmatter_name.clone(),
                    },
                );
                all_resolved.push(ResolvedImport {
                    identity: m.identity.clone(),
                    source: m.source.clone(),
                    decl_index: idx,
                    verdict: ImportVerdict::Included,
                });
            }
        }
    }

    ImportsResolution {
        skills: all_resolved,
        collisions,
        unknown_patterns,
        invalid_patterns,
    }
}

/// Walk one decl's patterns against its source's discovered set and
/// produce a per-skill verdict. Each matched skill carries a `MatchFate`
/// recording why filtering didn't drop it (or did).
fn match_decl(
    decl: &ImportDecl,
    decl_index: usize,
    discovered: &[Skill<Discovered>],
    unknown: &mut Vec<UnknownImportPattern>,
    invalid: &mut Vec<InvalidImportPattern>,
) -> Vec<MatchedSkill> {
    let mut out: Vec<MatchedSkill> = Vec::new();
    let mut seen: HashMap<Locator, usize> = HashMap::new();

    let allowed_tier = |tier: Tier| -> bool {
        match tier {
            Tier::Curated => true,
            Tier::Experimental => decl.include_experimental,
            Tier::System => decl.include_system,
        }
    };

    for pattern in decl.patterns() {
        if let Err(reason) = glob::validate(pattern) {
            invalid.push(InvalidImportPattern {
                source: decl.source.clone(),
                decl_index,
                pattern: pattern.to_string(),
                reason: reason.to_string(),
            });
            continue;
        }
        let pattern_is_explicit = !pattern.contains('*');
        let mut any_match = false;

        for d in discovered {
            if !pattern_matches(pattern, d.locator.as_str()) {
                continue;
            }
            any_match = true;

            // Resolve fate. Explicit-name imports bypass the tier and
            // internal filters (mirrors skills.sh).
            let fate = if !pattern_is_explicit && !allowed_tier(d.tier) {
                MatchFate::FilteredTier
            } else if d.internal && !decl.include_internal && !pattern_is_explicit {
                MatchFate::FilteredInternal
            } else if decl
                .exclude_skills
                .iter()
                .any(|ex| pattern_matches(ex, d.locator.as_str()))
            {
                MatchFate::ExcludedBySelf
            } else {
                MatchFate::Ok
            };

            let identity = d.locator.clone();
            if let Some(&existing) = seen.get(&identity) {
                // Same skill matched by multiple patterns within this
                // decl — promote `Ok` over filtered/excluded variants
                // (most permissive wins within a single decl).
                let existing_fate = &out[existing].fate;
                if matches!(existing_fate, MatchFate::Ok) {
                    continue;
                }
                if matches!(fate, MatchFate::Ok) {
                    out[existing].fate = fate;
                }
                continue;
            }
            seen.insert(identity.clone(), out.len());
            out.push(MatchedSkill {
                identity,
                source: decl.source.clone(),
                fate,
                frontmatter_name: d.frontmatter_name.clone(),
            });
        }

        if !any_match {
            unknown.push(UnknownImportPattern {
                source: decl.source.clone(),
                decl_index,
                pattern: pattern.to_string(),
            });
        }
    }

    out
}

fn decl_excludes_identity(decl: &ImportDecl, identity: &Locator) -> bool {
    decl.exclude_skills
        .iter()
        .any(|ex| pattern_matches(ex, identity.as_str()))
}

/// Intermediate per-decl record. Public verdicts live in `ResolvedImport`
/// after the cross-decl merge.
#[derive(Debug, Clone)]
struct MatchedSkill {
    identity: Locator,
    source: String,
    fate: MatchFate,
    frontmatter_name: Option<String>,
}

#[derive(Debug, Clone)]
enum MatchFate {
    Ok,
    FilteredInternal,
    FilteredTier,
    ExcludedBySelf,
}

struct ClaimEntry {
    decl_index: usize,
    source: String,
    frontmatter_name: Option<String>,
}

impl ImportsResolution {
    /// Identities admitted to the school under this resolution. Iterates
    /// in the order they were claimed.
    pub fn included(&self) -> impl Iterator<Item = &ResolvedImport> {
        self.skills
            .iter()
            .filter(|s| matches!(s.verdict, ImportVerdict::Included))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::Locator;
    use std::path::PathBuf;

    fn dskill(id: &str, tier: Tier, internal: bool, name: Option<&str>) -> Skill<Discovered> {
        Skill {
            locator: Locator::from_basename(id),
            path: PathBuf::from(format!("/src/{id}")),
            tier,
            internal,
            frontmatter_name: name.map(String::from),
            source: None,
            state: Discovered,
        }
    }

    fn decl(source: &str, skills: &[&str]) -> ImportDecl {
        ImportDecl {
            source: source.to_string(),
            skills: skills.iter().map(|s| s.to_string()).collect(),
            ..ImportDecl::default()
        }
    }

    fn discovery(entries: &[(&str, &[Skill<Discovered>])]) -> Discovery {
        let mut d = Discovery::default();
        for (k, v) in entries {
            d.insert(k, v.to_vec());
        }
        d
    }

    fn included_identities(r: &ImportsResolution) -> Vec<&str> {
        let mut out: Vec<&str> = r.included().map(|s| s.identity.as_str()).collect();
        out.sort();
        out
    }

    // -- single-decl basics --

    #[test]
    fn star_pattern_admits_all_curated() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("b", Tier::Experimental, false, None),
            dskill("c", Tier::System, false, None),
        ];
        let r = resolve_imports(
            &[decl("owner/repo", &["*"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        assert_eq!(included_identities(&r), vec!["a"]);
    }

    #[test]
    fn include_experimental_widens_glob() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("b", Tier::Experimental, false, None),
            dskill("c", Tier::System, false, None),
        ];
        let mut d = decl("owner/repo", &["*"]);
        d.include_experimental = true;
        let r = resolve_imports(&[d], &discovery(&[("owner/repo", src.as_slice())]));
        assert_eq!(included_identities(&r), vec!["a", "b"]);
    }

    #[test]
    fn include_system_widens_glob() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("c", Tier::System, false, None),
        ];
        let mut d = decl("owner/repo", &["*"]);
        d.include_system = true;
        let r = resolve_imports(&[d], &discovery(&[("owner/repo", src.as_slice())]));
        assert_eq!(included_identities(&r), vec!["a", "c"]);
    }

    #[test]
    fn explicit_name_bypasses_tier_filter() {
        let src = vec![
            dskill("shell", Tier::Experimental, false, None),
            dskill("skill-creator", Tier::System, false, None),
        ];
        let r = resolve_imports(
            &[decl("owner/repo", &["shell", "skill-creator"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        assert_eq!(included_identities(&r), vec!["shell", "skill-creator"]);
    }

    // -- internal handling --

    #[test]
    fn internal_skill_filtered_by_default_for_glob() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("b", Tier::Curated, true, None),
        ];
        let r = resolve_imports(
            &[decl("owner/repo", &["*"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        assert_eq!(included_identities(&r), vec!["a"]);
    }

    #[test]
    fn include_internal_admits_internal_via_glob() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("b", Tier::Curated, true, None),
        ];
        let mut d = decl("owner/repo", &["*"]);
        d.include_internal = true;
        let r = resolve_imports(&[d], &discovery(&[("owner/repo", src.as_slice())]));
        assert_eq!(included_identities(&r), vec!["a", "b"]);
    }

    #[test]
    fn internal_skill_admitted_via_explicit_name() {
        let src = vec![dskill("secret", Tier::Curated, true, None)];
        let r = resolve_imports(
            &[decl("owner/repo", &["secret"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        assert_eq!(included_identities(&r), vec!["secret"]);
    }

    // -- exclude_skills --

    #[test]
    fn exclude_skills_subtracts_from_match() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("b", Tier::Curated, false, None),
            dskill("c", Tier::Curated, false, None),
        ];
        let mut d = decl("owner/repo", &["*"]);
        d.exclude_skills = vec!["b".to_string()];
        let r = resolve_imports(&[d], &discovery(&[("owner/repo", src.as_slice())]));
        assert_eq!(included_identities(&r), vec!["a", "c"]);
    }

    // -- cross-source collision --

    #[test]
    fn first_decl_wins_on_identity_collision() {
        let src_a = vec![dskill("rust-coding", Tier::Curated, false, None)];
        let src_b = vec![dskill("rust-coding", Tier::Curated, false, None)];
        let r = resolve_imports(
            &[
                decl("upstream/a", &["rust-coding"]),
                decl("fork/b", &["rust-coding"]),
            ],
            &discovery(&[
                ("upstream/a", src_a.as_slice()),
                ("fork/b", src_b.as_slice()),
            ]),
        );
        assert_eq!(included_identities(&r), vec!["rust-coding"]);
        // The winner is the first-declared.
        let winner = r.included().next().unwrap();
        assert_eq!(winner.source, "upstream/a");

        assert_eq!(r.collisions.len(), 1);
        let c = &r.collisions[0];
        assert_eq!(c.identity, "rust-coding");
        assert_eq!(c.winner_source, "upstream/a");
        assert_eq!(c.loser_source, "fork/b");
        assert!(!c.suppressed_by_exclude);
    }

    #[test]
    fn exclude_on_winner_suppresses_collision_warning() {
        // Maintainer expressed intent: first decl excludes rust-coding,
        // second decl picks it up from a different source. No warning
        // because the maintainer signalled disjoint sets.
        let src_a = vec![dskill("rust-coding", Tier::Curated, false, None)];
        let src_b = vec![dskill("rust-coding", Tier::Curated, false, None)];
        let mut d_a = decl("upstream/a", &["*"]);
        d_a.exclude_skills = vec!["rust-coding".to_string()];
        let r = resolve_imports(
            &[d_a, decl("fork/b", &["rust-coding"])],
            &discovery(&[
                ("upstream/a", src_a.as_slice()),
                ("fork/b", src_b.as_slice()),
            ]),
        );
        // upstream/a excluded rust-coding → fork/b's rust-coding wins.
        let winner = r.included().find(|s| s.identity == "rust-coding").unwrap();
        assert_eq!(winner.source, "fork/b");
        // No collision: upstream/a didn't claim it.
        assert!(r.collisions.is_empty());
    }

    #[test]
    fn frontmatter_mismatch_recorded_at_collision() {
        let src_a = vec![dskill(
            "rust-coding",
            Tier::Curated,
            false,
            Some("rust-coding"),
        )];
        let src_b = vec![dskill(
            "rust-coding",
            Tier::Curated,
            false,
            Some("RustCoding"),
        )];
        let r = resolve_imports(
            &[
                decl("upstream/a", &["rust-coding"]),
                decl("fork/b", &["rust-coding"]),
            ],
            &discovery(&[
                ("upstream/a", src_a.as_slice()),
                ("fork/b", src_b.as_slice()),
            ]),
        );
        assert_eq!(r.collisions.len(), 1);
        assert!(r.collisions[0].frontmatter_mismatch);
    }

    #[test]
    fn matching_frontmatter_name_no_mismatch() {
        let src_a = vec![dskill(
            "rust-coding",
            Tier::Curated,
            false,
            Some("rust-coding"),
        )];
        let src_b = vec![dskill(
            "rust-coding",
            Tier::Curated,
            false,
            Some("rust-coding"),
        )];
        let r = resolve_imports(
            &[
                decl("upstream/a", &["rust-coding"]),
                decl("fork/b", &["rust-coding"]),
            ],
            &discovery(&[
                ("upstream/a", src_a.as_slice()),
                ("fork/b", src_b.as_slice()),
            ]),
        );
        assert!(!r.collisions[0].frontmatter_mismatch);
    }

    // -- nested identity --

    #[test]
    fn nested_identity_preserved_through_resolution() {
        let src = vec![Skill {
            locator: Locator::from_basename("typescript/coding"),
            path: PathBuf::from("/src"),
            tier: Tier::Curated,
            internal: false,
            frontmatter_name: None,
            source: None,
            state: Discovered,
        }];
        let r = resolve_imports(
            &[decl("owner/repo", &["*"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        assert_eq!(included_identities(&r), vec!["typescript/coding"]);
    }

    // -- unknown patterns --

    #[test]
    fn pattern_matching_nothing_recorded_as_unknown() {
        let src = vec![dskill("a", Tier::Curated, false, None)];
        let r = resolve_imports(
            &[decl("owner/repo", &["missing"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        assert!(included_identities(&r).is_empty());
        assert_eq!(r.unknown_patterns.len(), 1);
        assert_eq!(r.unknown_patterns[0].pattern, "missing");
    }

    #[test]
    fn missing_source_in_discovery_records_unknown_patterns() {
        let r = resolve_imports(&[decl("missing/source", &["a", "b"])], &discovery(&[]));
        assert_eq!(r.unknown_patterns.len(), 2);
    }

    // -- invalid glob syntax: warn-and-skip, never reject --

    #[test]
    fn invalid_import_pattern_recorded_and_skipped() {
        let src = vec![dskill("a", Tier::Curated, false, None)];
        let r = resolve_imports(
            &[decl("owner/repo", &["a?", "a"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        // `a?` is unsupported → invalid; the valid `a` still resolves.
        assert_eq!(r.invalid_patterns.len(), 1);
        assert_eq!(r.invalid_patterns[0].pattern, "a?");
        assert_eq!(r.invalid_patterns[0].decl_index, 0);
        assert_eq!(r.invalid_patterns[0].source, "owner/repo");
        assert_eq!(included_identities(&r), vec!["a"]);
        // Not double-reported as unknown.
        assert!(r.unknown_patterns.is_empty());
    }

    // -- multi-pattern decl --

    #[test]
    fn multi_pattern_decl_unions_matches() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("b", Tier::Curated, false, None),
            dskill("c", Tier::Curated, false, None),
        ];
        let r = resolve_imports(
            &[decl("owner/repo", &["a", "b"])],
            &discovery(&[("owner/repo", src.as_slice())]),
        );
        assert_eq!(included_identities(&r), vec!["a", "b"]);
    }

    // -- excluded-by-self verdict --

    #[test]
    fn excluded_by_self_appears_in_resolved_with_verdict() {
        let src = vec![
            dskill("a", Tier::Curated, false, None),
            dskill("b", Tier::Curated, false, None),
        ];
        let mut d = decl("owner/repo", &["*"]);
        d.exclude_skills = vec!["b".to_string()];
        let r = resolve_imports(&[d], &discovery(&[("owner/repo", src.as_slice())]));
        let b = r.skills.iter().find(|s| s.identity == "b").unwrap();
        assert_eq!(b.verdict, ImportVerdict::ExcludedBySelf);
    }
}
