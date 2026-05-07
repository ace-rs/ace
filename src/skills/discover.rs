use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Hidden directory names that mark tier sub-trees under `skills/`. Source of
/// truth for both `discover_skills` and any downstream code that needs to
/// distinguish a tier dir from a real skill dir (e.g. diff parsing).
pub const TIER_DIRS: &[&str] = &[".curated", ".experimental", ".system"];

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

#[derive(Clone)]
pub struct DiscoveredSkill {
    pub name: String,
    pub path: PathBuf,
    pub tier: Tier,
}

/// Discover skills under `<dir>/skills/`. Priority (first hit per name wins):
///
/// 1. `skills/.curated/<name>/`      → Tier::Curated
/// 2. `skills/<name>/`               → Tier::Curated
/// 3. `skills/.experimental/<name>/` → Tier::Experimental
/// 4. `skills/.system/<name>/`       → Tier::System
///
/// Collision between `.curated/` and top-level `skills/` resolves to `.curated/`.
/// Skills at the repo root (outside `skills/`) are not discovered.
pub fn discover_skills(dir: &Path) -> Result<Vec<DiscoveredSkill>, std::io::Error> {
    let mut skills = Vec::new();
    let mut seen = HashSet::new();

    let search = [
        (dir.join("skills/.curated"),      Tier::Curated),
        (dir.join("skills"),               Tier::Curated),
        (dir.join("skills/.experimental"), Tier::Experimental),
        (dir.join("skills/.system"),       Tier::System),
    ];

    for (path, tier) in &search {
        if path.is_dir() {
            scan_for_skills(path, *tier, &mut skills, &mut seen)?;
        }
    }

    Ok(skills)
}

fn scan_for_skills(
    parent: &Path,
    tier: Tier,
    skills: &mut Vec<DiscoveredSkill>,
    seen: &mut HashSet<String>,
) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(parent)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        if name.starts_with('.') {
            continue;
        }

        if path.join("SKILL.md").exists() && seen.insert(name.clone()) {
            skills.push(DiscoveredSkill { name, path, tier });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn empty_dir_returns_no_skills() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }

    #[test]
    fn files_in_skills_dir_are_skipped() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        fs::create_dir(tmp.path().join("skills")).expect("mkdir skills");
        fs::write(tmp.path().join("skills/loose.md"), "").expect("write file");
        fs::write(tmp.path().join("skills/SKILL.md"), "").expect("write SKILL.md");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty(), "regular files in skills/ should not be treated as skills");
    }

    #[test]
    fn dir_without_skill_md_is_skipped() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        fs::create_dir_all(tmp.path().join("skills/no-marker")).expect("create dir");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }

    #[test]
    fn finds_multiple_skills() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let skills_dir = tmp.path().join("skills");
        fs::create_dir(&skills_dir).expect("create skills dir");
        for name in ["alpha", "beta", "gamma"] {
            let d = skills_dir.join(name);
            fs::create_dir(&d).expect("create dir");
            fs::write(d.join("SKILL.md"), "").expect("write SKILL.md");
        }

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        let mut names: Vec<_> = skills.iter().map(|s| s.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta", "gamma"]);
    }

    // -- tier discovery (PROD9-75) --

    fn make_skill_at(base: &Path, rel: &str) -> PathBuf {
        let dir = base.join(rel);
        fs::create_dir_all(&dir).expect("create skill dir");
        fs::write(dir.join("SKILL.md"), "# skill").expect("write SKILL.md");
        dir
    }

    #[test]
    fn top_level_skill_tagged_curated() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/my-skill");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn finds_skill_in_curated_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = make_skill_at(tmp.path(), "skills/.curated/foo");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "foo");
        assert_eq!(skills[0].path, path);
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn finds_skill_in_experimental_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/.experimental/shell");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "shell");
        assert_eq!(skills[0].tier, Tier::Experimental);
    }

    #[test]
    fn finds_skill_in_system_subdir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/.system/skill-creator");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "skill-creator");
        assert_eq!(skills[0].tier, Tier::System);
    }

    #[test]
    fn curated_wins_over_top_level_on_collision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/dup");
        let curated = make_skill_at(tmp.path(), "skills/.curated/dup");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].path, curated, ".curated should win over top-level");
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn curated_wins_over_experimental_on_collision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let curated = make_skill_at(tmp.path(), "skills/.curated/ios-taste");
        make_skill_at(tmp.path(), "skills/.experimental/ios-taste");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].path, curated, ".curated should win over .experimental");
        assert_eq!(skills[0].tier, Tier::Curated);
    }

    #[test]
    fn experimental_wins_over_system_on_collision() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let experimental = make_skill_at(tmp.path(), "skills/.experimental/dup");
        make_skill_at(tmp.path(), "skills/.system/dup");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].path, experimental, ".experimental should win over .system");
        assert_eq!(skills[0].tier, Tier::Experimental);
    }

    #[test]
    fn different_tiers_coexist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/top");
        make_skill_at(tmp.path(), "skills/.curated/cur");
        make_skill_at(tmp.path(), "skills/.experimental/exp");
        make_skill_at(tmp.path(), "skills/.system/sys");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        let mut by_name: Vec<(&str, Tier)> =
            skills.iter().map(|s| (s.name.as_str(), s.tier)).collect();
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
        // Spec change: root-children scanning removed (PROD9-75).
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "orphan");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }

    #[test]
    fn hidden_non_tier_dirs_beneath_skills_are_skipped() {
        let tmp = tempfile::tempdir().expect("tempdir");
        make_skill_at(tmp.path(), "skills/.weird/thing");

        let skills = discover_skills(tmp.path()).expect("discover_skills");
        assert!(skills.is_empty());
    }
}
