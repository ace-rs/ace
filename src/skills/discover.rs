//! Skill discovery — find every `SKILL.md` under a source root and tag each
//! with identity, tier, and `internal` flag.
//!
//! See `docs/spec/skills/model.md` § Discovery Cascade and the two decision
//! docs dated 2026-05-26.
//!
//! Two-stage cascade:
//!
//! 1. **Direct skill** — `<root>/SKILL.md` exists → the root itself is a skill.
//!    Identity defaults to the basename of the root.
//!
//! 2. **Priority dirs (recursive within)** — walk each priority dir for
//!    `SKILL.md` at any depth. First-found wins on identity collisions
//!    across the stage. Within each priority dir, hidden subdirs are skipped
//!    (other than the recognized tier dirs, which are themselves priority
//!    entries).
//!
//!    Canonical priority order:
//!      - `skills/.curated/`         → Tier::Curated
//!      - `skills/`                  → Tier::Curated (tier subdirs excluded)
//!      - `skills/.experimental/`    → Tier::Experimental
//!      - `skills/.system/`          → Tier::System
//!
//!    Backend-fallback dirs (used only when the canonical entries yielded
//!    nothing): `.claude/skills/`, `.codex/skills/`, `.opencode/skills/`,
//!    `.cursor/skills/`, `.windsurf/skills/`, `.kiro/skills/`,
//!    `.agents/skills/`. All tagged Tier::Curated.
//!
//! Skills outside stage 1 or stage 2 priority dirs are off-spec and not
//! discovered (skills.sh's stage-3 whole-repo walk is deliberately dropped).

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::identity::Locator;
use super::name::RejectReason;
use super::{Discovered, Skill};

/// Hidden directory names that mark tier sub-trees under `skills/`. These are
/// processed as separate priority entries; the `skills/` walk excludes them
/// to avoid double-counting.
pub const TIER_DIRS: &[&str] = &[".curated", ".experimental", ".system"];

/// Backend-specific skill dirs, used as fallback when the canonical priority
/// entries yield no skills. Order is informational only — first-found wins
/// only within a single fallback dir; multiple fallback dirs together can
/// each contribute distinct identities.
pub const BACKEND_DIRS: &[&str] = &[
    ".claude/skills",
    ".codex/skills",
    ".opencode/skills",
    ".cursor/skills",
    ".windsurf/skills",
    ".kiro/skills",
    ".agents/skills",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tier {
    Curated,
    Experimental,
    System,
}

impl Tier {
    pub fn label(self) -> &'static str {
        match self {
            Tier::Curated => "curated",
            Tier::Experimental => "experimental",
            Tier::System => "system",
        }
    }
}

/// Build a freshly-discovered skill atom. Discovery is the only production
/// minter of `Skill<Discovered>` — the `Locator` is the prefix-strip rule's
/// output, and `source` starts empty (set later when the skill is pulled from
/// an import).
fn atom(
    locator: Locator,
    path: PathBuf,
    tier: Tier,
    internal: bool,
    frontmatter_name: Option<String>,
) -> Skill<Discovered> {
    Skill {
        locator,
        path,
        tier,
        internal,
        frontmatter_name,
        source: None,
        state: Discovered,
    }
}

/// Discover skills under `root` per the 2-stage cascade. See module docs.
///
/// Returns the discovered skills plus a list of *structural prunes*: paths that
/// looked like skills but whose identity failed structural validation at
/// `Locator` construction (e.g. a backslash segment). Discovery is `Ace`-less,
/// so it cannot warn directly — it hands each [`RejectReason`] (which carries the
/// offending name and the cause) back to the caller, which surfaces it. A prune
/// is a path-safety signal, not a skill verdict, so it is kept out of the skill
/// list rather than carried as a `Rejected`.
pub fn discover_skills(
    root: &Path,
) -> Result<(Vec<Skill<Discovered>>, Vec<RejectReason>), std::io::Error> {
    let mut prunes: Vec<RejectReason> = Vec::new();

    // Stage 1: direct skill at root.
    if root.join("SKILL.md").is_file() {
        let basename = root.file_name().and_then(|n| n.to_str()).unwrap_or("skill");
        match Locator::try_from_basename(basename) {
            Ok(locator) => {
                let (internal, frontmatter_name) = read_frontmatter_flags(&root.join("SKILL.md"));
                let skill = atom(
                    locator,
                    root.to_path_buf(),
                    Tier::Curated,
                    internal,
                    frontmatter_name,
                );
                return Ok((vec![skill], prunes));
            }
            Err(reason) => {
                prunes.push(reason);
                return Ok((Vec::new(), prunes));
            }
        }
    }

    let mut skills = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    // Stage 2 canonical entries. `skills/` walk excludes tier subdirs to
    // avoid double-counting with the dedicated tier entries.
    let canonical: &[(PathBuf, Tier, bool)] = &[
        (root.join("skills/.curated"), Tier::Curated, false),
        (root.join("skills"), Tier::Curated, true),
        (root.join("skills/.experimental"), Tier::Experimental, false),
        (root.join("skills/.system"), Tier::System, false),
    ];

    for (dir, tier, exclude_tier_subdirs) in canonical {
        if dir.is_dir() {
            walk_priority_dir(
                dir,
                dir,
                *tier,
                *exclude_tier_subdirs,
                &mut skills,
                &mut seen,
                &mut prunes,
            )?;
        }
    }

    // Stage 2 fallback: backend-specific dirs, only if canonical was empty.
    if skills.is_empty() {
        for backend_dir in BACKEND_DIRS {
            let dir = root.join(backend_dir);
            if dir.is_dir() {
                walk_priority_dir(
                    &dir,
                    &dir,
                    Tier::Curated,
                    false,
                    &mut skills,
                    &mut seen,
                    &mut prunes,
                )?;
            }
        }
    }

    Ok((skills, prunes))
}

/// Recursively walk a priority dir, collecting any directory that contains a
/// `SKILL.md`. The skill's identity is the parent dir's path relative to
/// `prefix` (the priority root), slash-joined.
///
/// Hidden subdirs are skipped. When `exclude_tier_subdirs` is true (the
/// canonical `skills/` walk), the top-level `.curated`/`.experimental`/
/// `.system` dirs are also skipped — they're walked by their own canonical
/// entries.
fn walk_priority_dir(
    dir: &Path,
    prefix: &Path,
    tier: Tier,
    exclude_tier_subdirs: bool,
    skills: &mut Vec<Skill<Discovered>>,
    seen: &mut HashSet<String>,
    prunes: &mut Vec<RejectReason>,
) -> Result<(), std::io::Error> {
    if dir.join("SKILL.md").is_file() {
        let rel = match dir.strip_prefix(prefix) {
            Ok(rel) if !rel.as_os_str().is_empty() => rel,
            _ => return Ok(()), // priority root itself is not a skill; stage 1 territory
        };
        // Structural path-safety prune: record the reason for the caller to
        // surface as a warning, and skip this dir.
        let locator = match Locator::try_from_path(rel) {
            Ok(locator) => locator,
            Err(reason) => {
                prunes.push(reason);
                return Ok(());
            }
        };
        if seen.insert(locator.as_str().to_string()) {
            let (internal, frontmatter_name) = read_frontmatter_flags(&dir.join("SKILL.md"));
            skills.push(atom(
                locator,
                dir.to_path_buf(),
                tier,
                internal,
                frontmatter_name,
            ));
        }
        // Skills cannot nest inside other skills; don't recurse into a dir
        // that's already a skill. Matches skills.sh behavior.
        return Ok(());
    }

    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if !file_type.is_dir() {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if name.starts_with('.') {
            // Tier dirs at the top of `skills/` are handled by their own
            // canonical entries — skip to avoid duplicates.
            if exclude_tier_subdirs && dir == prefix && TIER_DIRS.contains(&name) {
                continue;
            }
            // Other hidden dirs (.git, .venv, etc.) are skipped wholesale.
            continue;
        }
        if SKIP_DIRS.contains(&name) {
            continue;
        }
        walk_priority_dir(
            &path,
            prefix,
            tier,
            exclude_tier_subdirs,
            skills,
            seen,
            prunes,
        )?;
    }
    Ok(())
}

/// Dirs we never recurse into. Matches skills.sh's defaults plus a few
/// ecosystem extensions to keep recursive walks bounded.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "__pycache__",
    "target",
    ".venv",
    ".next",
    ".turbo",
    "out",
    "vendor",
];

/// Best-effort frontmatter extraction for the fields discovery cares
/// about: `internal: true` (discovery-time filter) and `name:` (used by
/// the imports resolver to flag cross-source divergence at colliding
/// identities). A full frontmatter parser lives in `skills::meta`;
/// this is a narrow read so discovery doesn't have to materialize the
/// full struct for every skill on disk.
///
/// Returns `(internal, frontmatter_name)`. Either or both may be absent
/// from the SKILL.md; the function never errors.
fn read_frontmatter_flags(skill_md: &Path) -> (bool, Option<String>) {
    let Ok(content) = std::fs::read_to_string(skill_md) else {
        return (false, None);
    };
    let content = content.trim_start();
    let Some(rest) = content.strip_prefix("---") else {
        return (false, None);
    };
    let Some(close) = rest.find("\n---") else {
        return (false, None);
    };
    let block = &rest[..close];
    let mut internal = false;
    let mut name: Option<String> = None;
    for line in block.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("internal:") {
            let val = val
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_ascii_lowercase();
            internal = val == "true";
        } else if let Some(val) = trimmed.strip_prefix("name:") {
            let val = val.trim().trim_matches('"').trim_matches('\'');
            if !val.is_empty() {
                name = Some(val.to_string());
            }
        }
    }
    (internal, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_skill_at(base: &Path, rel: &str) -> PathBuf {
        let dir = base.join(rel);
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(dir.join("SKILL.md"), "# skill").expect("write SKILL.md");
        dir
    }

    fn make_skill_with_frontmatter(base: &Path, rel: &str, frontmatter: &str) -> PathBuf {
        let dir = base.join(rel);
        fs::create_dir_all(&dir).expect("create skill dir");
        let body = format!("---\n{frontmatter}\n---\n# skill\n");
        fs::write(dir.join("SKILL.md"), body).expect("write SKILL.md");
        dir
    }

    // -- preserved tests --

    #[test]
    fn empty_dir_returns_no_skills() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }

    #[test]
    fn files_in_skills_dir_are_skipped() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        fs::create_dir(tmp.path().join("skills")).expect("mkdir skills");
        fs::write(tmp.path().join("skills/loose.md"), "").expect("write file");
        fs::write(tmp.path().join("skills/SKILL.md"), "").expect("write SKILL.md");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        // The `skills/SKILL.md` at the top of the priority dir is stage-1
        // territory for the `skills/` root, but we deliberately ignore an
        // empty identity (priority root itself). Result: no skills.
        assert!(
            skills.is_empty(),
            "files in skills/ should not be treated as skills"
        );
    }

    #[test]
    fn dir_without_skill_md_is_skipped() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        fs::create_dir_all(tmp.path().join("skills/no-marker")).expect("create dir");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }

    #[test]
    fn finds_multiple_skills() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        for name in ["alpha", "beta", "gamma"] {
            make_skill_at(tmp.path(), &format!("skills/{name}"));
        }

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        let mut names: Vec<_> = skills.iter().map(|s| s.locator.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn top_level_skill_tagged_curated() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/my-skill");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn finds_skill_in_curated_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = make_skill_at(tmp.path(), "skills/.curated/foo");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].locator, "foo");
        assert_eq!(skills[0].path, path);
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn finds_skill_in_experimental_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/.experimental/shell");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].locator, "shell");
        assert_eq!(skills[0].tier, Tier::Experimental);
    }

    #[test]
    fn finds_skill_in_system_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/.system/skill-creator");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].locator, "skill-creator");
        assert_eq!(skills[0].tier, Tier::System);
    }

    #[test]
    fn curated_wins_over_top_level_on_collision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/dup");
        let curated = make_skill_at(tmp.path(), "skills/.curated/dup");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(
            skills[0].path, curated,
            ".curated should win over top-level"
        );
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn curated_wins_over_experimental_on_collision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let curated = make_skill_at(tmp.path(), "skills/.curated/ios-taste");
        make_skill_at(tmp.path(), "skills/.experimental/ios-taste");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(
            skills[0].path, curated,
            ".curated should win over .experimental"
        );
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn experimental_wins_over_system_on_collision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let experimental = make_skill_at(tmp.path(), "skills/.experimental/dup");
        make_skill_at(tmp.path(), "skills/.system/dup");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(
            skills[0].path, experimental,
            ".experimental should win over .system"
        );
        assert_eq!(skills[0].tier, Tier::Experimental);
    }

    #[test]
    fn different_tiers_coexist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/top");
        make_skill_at(tmp.path(), "skills/.curated/cur");
        make_skill_at(tmp.path(), "skills/.experimental/exp");
        make_skill_at(tmp.path(), "skills/.system/sys");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        let mut by_name: Vec<(&str, Tier)> = skills
            .iter()
            .map(|s| (s.locator.as_str(), s.tier))
            .collect();
        by_name.sort_by_key(|(n, _)| *n);

        assert_eq!(
            by_name,
            vec![
                ("cur", Tier::Curated),
                ("exp", Tier::Experimental),
                ("sys", Tier::System),
                ("top", Tier::Curated),
            ]
        );
    }

    #[test]
    fn root_level_skill_outside_skills_dir_is_not_discovered() {
        // No SKILL.md at root, an orphan SKILL.md at <root>/orphan/.
        // Neither stage 1 (root SKILL.md) nor stage 2 (under skills/) fires.
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "orphan");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }

    #[test]
    fn hidden_non_tier_dirs_beneath_skills_are_skipped() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/.weird/thing");

        let (skills, _) = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }

    // -- new tests for the 2-stage cascade --

    #[test]
    fn stage1_root_skill_md_is_discovered() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("my-repo");
        fs::create_dir_all(&root).expect("mkdir root");
        fs::write(root.join("SKILL.md"), "# root skill").expect("write");

        let (skills, _) = discover_skills(&root).expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].locator, "my-repo");
        assert_eq!(skills[0].path, root);
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn stage1_short_circuits_stage2() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("mono");
        fs::create_dir_all(&root).expect("mkdir");
        fs::write(root.join("SKILL.md"), "# root skill").expect("write root SKILL.md");
        make_skill_at(&root, "skills/inner");

        let (skills, _) = discover_skills(&root).expect("discover");
        assert_eq!(skills.len(), 1, "stage 1 must short-circuit");
        assert_eq!(skills[0].locator, "mono");
    }

    #[test]
    fn nested_layout_under_canonical_skills_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/typescript/coding");
        make_skill_at(tmp.path(), "skills/rust/coding");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        let mut names: Vec<_> = skills.iter().map(|s| s.locator.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["rust/coding", "typescript/coding"]);
        for s in &skills {
            assert_eq!(s.tier, Tier::Curated);
        }
    }

    #[test]
    fn nested_layout_under_curated_tier() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/.curated/group/leaf");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].locator, "group/leaf");
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn backend_fallback_dir_finds_skills_when_canonical_empty() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), ".claude/skills/foo");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].locator, "foo");
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn backend_fallback_supports_nested_layout() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), ".codex/skills/typescript/coding");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].locator, "typescript/coding");
    }

    #[test]
    fn backend_fallback_silenced_when_canonical_has_skills() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let canonical = make_skill_at(tmp.path(), "skills/foo");
        make_skill_at(tmp.path(), ".claude/skills/foo");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        assert_eq!(
            skills.len(),
            1,
            "fallback must be silenced when canonical yields skills"
        );
        assert_eq!(skills[0].path, canonical);
    }

    #[test]
    fn multiple_backend_dirs_can_each_contribute() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), ".claude/skills/alpha");
        make_skill_at(tmp.path(), ".codex/skills/beta");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        let mut names: Vec<_> = skills.iter().map(|s| s.locator.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn skip_dirs_not_recursed() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/node_modules/junk");
        make_skill_at(tmp.path(), "skills/target/junk");
        make_skill_at(tmp.path(), "skills/real");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        let names: Vec<_> = skills.iter().map(|s| s.locator.as_str()).collect();
        assert_eq!(names, vec!["real"]);
    }

    #[test]
    fn structurally_unsafe_identity_is_pruned_with_reason() {
        // A backslash in a path component is structurally unsafe (Windows path
        // separator). Discovery prunes it AND returns the reason, so the caller
        // can surface a warning — it must never be silently dropped.
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/foo\\bar");
        make_skill_at(tmp.path(), "skills/good");

        let (skills, prunes) = discover_skills(tmp.path()).expect("discover");
        let names: Vec<_> = skills.iter().map(|s| s.locator.as_str()).collect();
        assert_eq!(names, vec!["good"]);
        assert_eq!(prunes.len(), 1);
        assert!(matches!(prunes[0], RejectReason::Backslash { .. }));
    }

    #[test]
    fn internal_flag_parsed_from_frontmatter() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_with_frontmatter(
            tmp.path(),
            "skills/secret",
            "name: secret\ndescription: hidden\ninternal: true",
        );

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        assert_eq!(skills.len(), 1);
        assert!(skills[0].internal, "internal: true should be detected");
    }

    #[test]
    fn internal_flag_defaults_false() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/public");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        assert_eq!(skills.len(), 1);
        assert!(!skills[0].internal);
    }

    #[test]
    fn skill_does_not_recurse_into_itself() {
        // A SKILL.md at skills/foo/ should not also pick up skills/foo/sub/SKILL.md
        // as a separate skill — skills cannot nest.
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/foo");
        make_skill_at(tmp.path(), "skills/foo/sub");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        let names: Vec<_> = skills.iter().map(|s| s.locator.as_str()).collect();
        assert_eq!(names, vec!["foo"]);
    }

    #[test]
    fn nested_identity_strips_priority_prefix_correctly() {
        // Same leaf name under different parents → distinct identities,
        // no collision.
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/python/coding");
        make_skill_at(tmp.path(), "skills/rust/coding");

        let (skills, _) = discover_skills(tmp.path()).expect("discover");
        let mut names: Vec<_> = skills.iter().map(|s| s.locator.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["python/coding", "rust/coding"]);
    }

    fn discovered_with_name(id: &str, frontmatter_name: Option<&str>) -> Skill<Discovered> {
        atom(
            Locator::from_basename(id),
            PathBuf::from(format!("/s/{id}")),
            Tier::Curated,
            false,
            frontmatter_name.map(String::from),
        )
    }

    #[test]
    fn frontmatter_warning_flags_spoofable_name_without_rejecting() {
        // Clean identity, hostile frontmatter `name:` — admitted, but warned.
        let skill = discovered_with_name("coding", Some("evil\u{202E}exe"));
        assert!(skill.admission().is_ok());
        let warning = skill.frontmatter_warning().expect("warning expected");
        assert!(warning.contains("coding"));
        assert!(warning.contains("U+202E"));
    }

    #[test]
    fn frontmatter_warning_silent_on_clean_or_absent_name() {
        assert!(
            discovered_with_name("coding", Some("ts-coding"))
                .frontmatter_warning()
                .is_none()
        );
        assert!(
            discovered_with_name("coding", None)
                .frontmatter_warning()
                .is_none()
        );
    }
}
