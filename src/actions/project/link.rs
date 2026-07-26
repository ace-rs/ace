use std::path::Path;

use crate::ace::Ace;
use crate::actions::project::PrepareError;
use crate::actions::project::link_skills::{self, DesiredLink, create_dir_symlink, is_symlink};

/// Folders that ACE links from the school clone into the project.
pub const SCHOOL_FOLDERS: &[&str] = &["skills", "rules", "commands", "agents"];

/// Symlink school folders from cache into the project.
///
/// `skills` is special: rather than a whole-dir symlink, the skills folder
/// becomes a real dir populated with per-skill symlinks driven by `skills`.
/// The other folders (rules/commands/agents) remain whole-dir symlinks.
pub struct Link<'a> {
    pub school_root: &'a Path,
    pub project_dir: &'a Path,
    pub backend_dir: &'a str,
    /// Per-skill (name, target) pairs for the skills folder. Computed by
    /// the caller from discovery + resolver. Empty = no skills linked
    /// (skills dir exists but is empty).
    pub skills: &'a [DesiredLink],
    /// Parent of all school clones (`~/.local/share/ace/`). Used as the
    /// managed-symlink predicate root so a school switch via `ace.toml`
    /// prunes leftover per-skill links pointing at the previous clone.
    pub ace_data_root: &'a Path,
    /// Authorization to replace symlinks pointing outside every managed root.
    /// Without it, such a link at a desired skill's name fails the run.
    pub force: link_skills::Force,
}

impl Link<'_> {
    pub fn run(&self, _ace: &mut Ace) -> Result<LinkResult, PrepareError> {
        let mut folders = Vec::new();
        let mut skill_warnings = Vec::new();

        for &name in SCHOOL_FOLDERS {
            let school_dir = self.school_root.join(name);
            if !school_dir.exists() {
                continue;
            }

            let project_dir = self.project_dir.join(self.backend_dir).join(name);

            if name == "skills" {
                let result = link_skills::reconcile(
                    self.school_root,
                    self.ace_data_root,
                    &project_dir,
                    self.skills,
                    self.force,
                )
                .map_err(PrepareError::Write)?;
                if !result.blocked.is_empty() {
                    return Err(PrepareError::BlockedLinks(result.blocked));
                }
                folders.push(FolderResult {
                    name,
                    linked: result.changed(),
                    adopted: false,
                });
                skill_warnings.extend(result.warnings);
                continue;
            }

            let previous_dir = self
                .project_dir
                .join(self.backend_dir)
                .join(format!("previous-{name}"));
            let adopted = adopt_previous(&project_dir, &previous_dir)?;
            let linked = ensure_symlink(&project_dir, &school_dir)?;
            folders.push(FolderResult {
                name,
                linked,
                adopted,
            });
        }

        Ok(LinkResult {
            folders,
            skill_warnings,
        })
    }
}

#[derive(Debug, Default)]
pub struct LinkResult {
    pub folders: Vec<FolderResult>,
    /// Per-skill warnings (e.g. foreign entries blocking a managed link).
    /// Caller surfaces these via `ace.warn`.
    pub skill_warnings: Vec<String>,
}

#[derive(Debug)]
pub struct FolderResult {
    pub name: &'static str,
    pub linked: bool,
    pub adopted: bool,
}

#[cfg(test)]
impl LinkResult {
    pub fn linked(&self, name: &str) -> bool {
        self.folders.iter().any(|f| f.name == name && f.linked)
    }

    pub fn adopted(&self, name: &str) -> bool {
        self.folders.iter().any(|f| f.name == name && f.adopted)
    }
}

/// Create or update the folder-level symlink from `link_path` to `target`.
fn ensure_symlink(link_path: &Path, target: &Path) -> Result<bool, PrepareError> {
    match link_status(link_path) {
        LinkStatus::Symlink(current) if current == target => return Ok(false),
        LinkStatus::Symlink(_) => {
            std::fs::remove_file(link_path).map_err(PrepareError::Write)?;
        }
        LinkStatus::RealDir => {
            return Err(PrepareError::Write(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "{} is a real directory — adoption should have renamed it away",
                    link_path.display()
                ),
            )));
        }
        LinkStatus::Absent => {
            if let Some(parent) = link_path.parent() {
                std::fs::create_dir_all(parent).map_err(PrepareError::Write)?;
            }
        }
    }

    create_dir_symlink(target, link_path).map_err(PrepareError::Write)?;
    Ok(true)
}

/// On first setup (no symlinks yet), rename the whole dir to `previous-{name}/`.
fn adopt_previous(project_dir: &Path, previous_dir: &Path) -> Result<bool, PrepareError> {
    // Symlink or absent — nothing to adopt.
    if is_symlink(project_dir) || !project_dir.exists() {
        return Ok(false);
    }

    // It's a real directory — rename it wholesale.
    if previous_dir.exists() {
        return Err(PrepareError::Write(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!(
                "{} already exists — consolidate or remove it first",
                previous_dir.display()
            ),
        )));
    }

    std::fs::rename(project_dir, previous_dir).map_err(PrepareError::Write)?;
    Ok(true)
}

enum LinkStatus {
    Absent,
    Symlink(std::path::PathBuf),
    RealDir,
}

fn link_status(path: &Path) -> LinkStatus {
    if is_symlink(path) {
        let target = std::fs::read_link(path).unwrap_or_default();
        LinkStatus::Symlink(target)
    } else if path.exists() {
        LinkStatus::RealDir
    } else {
        LinkStatus::Absent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    struct TestFixture {
        _tmp: TempDir,
        root: PathBuf,
    }

    impl TestFixture {
        fn new() -> Self {
            let tmp = tempfile::tempdir().expect("create tempdir");
            let root = tmp.path().to_path_buf();
            let fix = Self { _tmp: tmp, root };
            std::fs::create_dir_all(fix.school()).expect("create school dir");
            std::fs::create_dir_all(fix.project()).expect("create project dir");
            fix
        }

        fn school(&self) -> PathBuf {
            self.root.join("school")
        }
        fn project(&self) -> PathBuf {
            self.root.join("project")
        }

        fn school_folder(&self, name: &str) -> PathBuf {
            self.school().join(name)
        }
        fn project_folder(&self, name: &str) -> PathBuf {
            self.project().join(".claude").join(name)
        }
        fn previous_folder(&self, name: &str) -> PathBuf {
            self.project()
                .join(".claude")
                .join(format!("previous-{name}"))
        }

        fn add_school_entry(&self, folder: &str, name: &str) {
            std::fs::create_dir_all(self.school_folder(folder).join(name))
                .expect("create school entry dir");
        }

        fn add_school_entry_with_content(
            &self,
            folder: &str,
            name: &str,
            file: &str,
            content: &str,
        ) {
            let dir = self.school_folder(folder).join(name);
            std::fs::create_dir_all(&dir).expect("create school entry dir");
            std::fs::write(dir.join(file), content).expect("write entry file");
        }

        fn add_real_entry(&self, folder: &str, name: &str, file: &str, content: &str) {
            let dir = self.project_folder(folder).join(name);
            std::fs::create_dir_all(&dir).expect("create real entry dir");
            std::fs::write(dir.join(file), content).expect("write entry file");
        }

        fn link(&self) -> Result<LinkResult, PrepareError> {
            link_all(&self.school(), &self.project(), &[])
        }

        fn link_with_skills(&self, skills: &[DesiredLink]) -> Result<LinkResult, PrepareError> {
            link_all(&self.school(), &self.project(), skills)
        }
    }

    #[test]
    fn creates_directory_symlink() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let target = tmp.path().join("target");
        std::fs::create_dir(&target).expect("create target dir");
        let link = tmp.path().join("link");
        create_dir_symlink(&target, &link).expect("create symlink");
        assert!(link.exists(), "symlink must exist");
        assert!(link.is_dir(), "symlink must resolve to directory");
    }

    #[test]
    fn fixtures_are_isolated_per_call() {
        let a = TestFixture::new();
        let b = TestFixture::new();
        assert_ne!(a.root, b.root, "fixtures must be isolated between calls");
    }

    #[test]
    fn link_no_school_folders() {
        let fix = TestFixture::new();
        let result = fix.link().expect("should succeed");
        assert!(result.folders.is_empty());
    }

    // Non-skills folders still use whole-dir symlinks.

    #[test]
    fn link_creates_whole_dir_symlink_for_rules() {
        let fix = TestFixture::new();
        fix.add_school_entry_with_content("rules", "indent", "rule.md", "# Indent");

        let result = fix.link().expect("should create symlink");
        assert!(result.linked("rules"));

        let link = fix.project_folder("rules");
        assert!(
            link.symlink_metadata()
                .expect("link exists")
                .file_type()
                .is_symlink()
        );

        let target = std::fs::read_link(&link).expect("read link");
        assert_eq!(target, fix.school_folder("rules"));
    }

    #[test]
    fn link_skips_correct_symlink_for_rules() {
        let fix = TestFixture::new();
        fix.add_school_entry("rules", "indent");

        let project_rules = fix.project_folder("rules");
        std::fs::create_dir_all(project_rules.parent().expect("has parent")).expect("mkdir parent");
        create_dir_symlink(&fix.school_folder("rules"), &project_rules).expect("create symlink");

        let result = fix.link().expect("should skip existing");
        assert!(!result.linked("rules"));
    }

    #[test]
    fn link_replaces_stale_symlink_for_rules() {
        let fix = TestFixture::new();
        fix.add_school_entry("rules", "indent");

        let project_rules = fix.project_folder("rules");
        std::fs::create_dir_all(project_rules.parent().expect("has parent")).expect("mkdir parent");
        create_dir_symlink(&fix.root.join("nonexistent"), &project_rules)
            .expect("create stale symlink");

        let result = fix.link().expect("should replace stale");
        assert!(result.linked("rules"));

        let target = std::fs::read_link(&project_rules).expect("read link");
        assert_eq!(target, fix.school_folder("rules"));
    }

    #[test]
    fn rules_dir_is_adopted_on_first_setup() {
        let fix = TestFixture::new();
        fix.add_school_entry("rules", "school-rule");
        fix.add_real_entry("rules", "my-rule", "rule.md", "# My");

        let result = fix.link().expect("should adopt and link");
        assert!(result.linked("rules"));
        assert!(result.adopted("rules"));

        let prev = fix.previous_folder("rules").join("my-rule").join("rule.md");
        let content = std::fs::read_to_string(prev).expect("moved rule should exist");
        assert_eq!(content, "# My");
    }

    // Skills folder uses per-skill reconciliation (no whole-dir symlink).

    #[test]
    fn link_skills_creates_real_dir_with_per_skill_symlinks() {
        let fix = TestFixture::new();
        fix.add_school_entry_with_content("skills", "rust-coding", "SKILL.md", "# Rust");
        fix.add_school_entry_with_content("skills", "go-coding", "SKILL.md", "# Go");

        let desired = vec![
            DesiredLink {
                name: "rust-coding".to_string(),
                target: fix.school_folder("skills").join("rust-coding"),
            },
            DesiredLink {
                name: "go-coding".to_string(),
                target: fix.school_folder("skills").join("go-coding"),
            },
        ];

        let result = fix.link_with_skills(&desired).expect("link should succeed");
        assert!(result.linked("skills"));

        let skills_dir = fix.project_folder("skills");
        assert!(skills_dir.is_dir(), "skills should be a real dir");
        assert!(
            !skills_dir
                .symlink_metadata()
                .expect("exists")
                .file_type()
                .is_symlink(),
            "skills dir must not be a symlink"
        );

        for name in ["rust-coding", "go-coding"] {
            let link = skills_dir.join(name);
            assert!(
                link.symlink_metadata()
                    .expect("exists")
                    .file_type()
                    .is_symlink(),
                "{name} should be a symlink"
            );
            let content =
                std::fs::read_to_string(link.join("SKILL.md")).expect("read through link");
            assert!(!content.is_empty());
        }
    }

    #[test]
    fn link_skills_migrates_legacy_whole_dir_symlink() {
        let fix = TestFixture::new();
        fix.add_school_entry_with_content("skills", "rust-coding", "SKILL.md", "# Rust");

        // Pre-step-3 layout: single symlink for the whole skills folder.
        let project_skills = fix.project_folder("skills");
        std::fs::create_dir_all(project_skills.parent().expect("has parent"))
            .expect("mkdir parent");
        create_dir_symlink(&fix.school_folder("skills"), &project_skills)
            .expect("create legacy symlink");

        let desired = vec![DesiredLink {
            name: "rust-coding".to_string(),
            target: fix.school_folder("skills").join("rust-coding"),
        }];

        let result = fix
            .link_with_skills(&desired)
            .expect("migration should succeed");
        assert!(result.linked("skills"));

        assert!(
            !project_skills
                .symlink_metadata()
                .expect("exists")
                .file_type()
                .is_symlink(),
            "legacy symlink must have been migrated to a real dir"
        );

        let per_skill = project_skills.join("rust-coding");
        assert!(
            per_skill
                .symlink_metadata()
                .expect("exists")
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn link_skills_warns_on_foreign_entry() {
        let fix = TestFixture::new();
        fix.add_school_entry_with_content("skills", "rust-coding", "SKILL.md", "# Rust");

        // User dropped a real dir at the colliding name.
        let user_dir = fix.project_folder("skills").join("rust-coding");
        std::fs::create_dir_all(&user_dir).expect("mkdir user dir");

        let desired = vec![DesiredLink {
            name: "rust-coding".to_string(),
            target: fix.school_folder("skills").join("rust-coding"),
        }];

        let result = fix.link_with_skills(&desired).expect("should soft-fail");
        assert_eq!(result.skill_warnings.len(), 1);
        assert!(result.skill_warnings[0].contains("rust-coding"));
        assert!(result.skill_warnings[0].contains("not managed"));
    }

    #[test]
    fn link_all_four_folders_with_one_skill() {
        let fix = TestFixture::new();
        for folder in SCHOOL_FOLDERS {
            fix.add_school_entry(folder, "entry");
        }

        let desired = vec![DesiredLink {
            name: "entry".to_string(),
            target: fix.school_folder("skills").join("entry"),
        }];

        let result = fix.link_with_skills(&desired).expect("should link all");
        assert_eq!(result.folders.len(), 4);
        for folder in SCHOOL_FOLDERS {
            assert!(result.linked(folder), "{folder} should be linked");
        }
    }

    #[test]
    fn link_skips_absent_folders() {
        let fix = TestFixture::new();
        fix.add_school_entry("skills", "a-skill");
        fix.add_school_entry("commands", "a-command");

        let desired = vec![DesiredLink {
            name: "a-skill".to_string(),
            target: fix.school_folder("skills").join("a-skill"),
        }];

        let result = fix.link_with_skills(&desired).expect("should link partial");
        assert_eq!(result.folders.len(), 2);
        assert!(result.linked("skills"));
        assert!(result.linked("commands"));
        assert!(!result.linked("rules"));
        assert!(!result.linked("agents"));
    }

    /// Helper — mirrors `Link::run` for tests that don't construct an Ace instance.
    fn link_all(
        school_root: &Path,
        project_dir: &Path,
        skills: &[DesiredLink],
    ) -> Result<LinkResult, PrepareError> {
        let mut folders = Vec::new();
        let mut skill_warnings = Vec::new();

        for &name in SCHOOL_FOLDERS {
            let school_dir = school_root.join(name);
            if !school_dir.exists() {
                continue;
            }

            let proj_dir = project_dir.join(".claude").join(name);

            if name == "skills" {
                // Test helper uses `school_root` for both managed-root args;
                // all skill targets live under it, so the prefix check still
                // classifies managed links correctly without a real data dir.
                let result = link_skills::reconcile(
                    school_root,
                    school_root,
                    &proj_dir,
                    skills,
                    link_skills::Force::No,
                )
                .map_err(PrepareError::Write)?;
                folders.push(FolderResult {
                    name,
                    linked: result.changed(),
                    adopted: false,
                });
                skill_warnings.extend(result.warnings);
                continue;
            }

            let prev_dir = project_dir.join(".claude").join(format!("previous-{name}"));
            let adopted = adopt_previous(&proj_dir, &prev_dir)?;
            let linked = ensure_symlink(&proj_dir, &school_dir)?;
            folders.push(FolderResult {
                name,
                linked,
                adopted,
            });
        }

        Ok(LinkResult {
            folders,
            skill_warnings,
        })
    }
}
