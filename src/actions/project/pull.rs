use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use crate::ace::Ace;
use crate::actions::project::PrepareError;
use crate::school::linked::LinkedSchool;

pub use crate::skills::{ChangeKind, SkillChange};

const FETCH_COOLDOWN: Duration = Duration::from_secs(15 * 60);

/// Outcome of a school clone update — carries data for the caller to act on.
#[derive(Debug)]
pub enum PullOutcome {
    /// Embedded school, no cache to update.
    Embedded,
    /// Cache is fresh (within cooldown), no fetch needed.
    Fresh,
    /// Was on a non-main branch (clean), switched back to main.
    SwitchedBranch { from: String },
    /// Fetched and fast-forwarded successfully.
    Updated { changes: Vec<SkillChange> },
    /// Working tree has uncommitted changes.
    Dirty { on_main: bool, branch: String },
    /// Local commits ahead of origin (can't fast-forward).
    AheadOfOrigin { clone_path: String },
    /// Local and remote have diverged (ff-only merge failed).
    Diverged { error: String },
}

impl PullOutcome {
    pub fn emit(&self, ace: &mut Ace) {
        match self {
            PullOutcome::Embedded => ace.info("Embedded school — nothing to pull."),
            PullOutcome::Fresh => ace.info("School is up to date."),
            PullOutcome::SwitchedBranch { from } => {
                ace.hint(&format!(
                    "Switched school clone from branch {from} back to main"
                ));
            }
            PullOutcome::Updated { changes } => {
                ace.done(&crate::skills::format_pull_summary(changes));

                // Pull updates the clone only — `ace link` owns the project's
                // symlinks. New or removed skills are not live until it runs.
                if !changes.is_empty() {
                    ace.hint("Run 'ace link' to update this project's skill links");
                }
            }
            PullOutcome::Dirty { on_main: true, .. } => {
                ace.warn("school has local changes — updates blocked");
                ace.hint("Skills may be outdated until changes are proposed.");
                ace.hint("Ask your AI assistant to propose the changes — it knows how.");
            }
            PullOutcome::Dirty {
                on_main: false,
                branch,
            } => {
                ace.warn(&format!(
                    "school is on branch {branch} with uncommitted changes — updates blocked"
                ));
                ace.hint("Skills may be outdated. Ask your AI assistant to propose the changes — it knows how.");
            }
            PullOutcome::AheadOfOrigin { clone_path } => {
                ace.warn(&format!("school has local commits at {clone_path}"));
                ace.hint("Propose changes back to the school repo, or resolve manually.");
            }
            PullOutcome::Diverged { error } => {
                ace.warn(&format!("school has diverged from origin/main: {error}"));
                ace.hint("Propose changes back to the school repo, or resolve manually.");
            }
        }
    }
}

/// Git fetch + ff-only merge school clone to latest origin/main.
/// Dirty, ahead, or diverged clones are warned but not errors — update is skipped.
pub struct Pull<'a> {
    pub school: &'a LinkedSchool,
    pub force: bool,
}

impl Pull<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<PullOutcome, PrepareError> {
        let Some(clone_path) = &self.school.clone_path else {
            return Ok(PullOutcome::Embedded);
        };

        if !clone_path.join(".git").exists() {
            return Err(PrepareError::Clone(format!(
                "school not installed: {}",
                self.school.raw_specifier
            )));
        }

        // -- ensure working tree is clean and on main --

        let git = ace.git(clone_path);
        let branch = git
            .current_branch()
            .map_err(|e| PrepareError::Clone(e.to_string()))?;
        let on_main = branch == "main";
        let dirty = git
            .is_dirty()
            .map_err(|e| PrepareError::Clone(e.to_string()))?;

        if dirty {
            return Ok(PullOutcome::Dirty {
                on_main,
                branch: branch.to_string(),
            });
        }

        let switched_from = if !on_main {
            let from = branch.clone();
            git.checkout_branch("main")
                .map_err(|e| PrepareError::Clone(e.to_string()))?;
            Some(from)
        } else {
            None
        };

        if !self.force && !is_stale(clone_path) {
            warn_unusable_skill_folders(ace, clone_path);
            return if let Some(from) = switched_from {
                Ok(PullOutcome::SwitchedBranch { from })
            } else {
                Ok(PullOutcome::Fresh)
            };
        }

        // -- fetch and fast-forward --

        let old_head = git
            .rev_parse("HEAD")
            .map_err(|e| PrepareError::Clone(e.to_string()))?;

        ace.progress(&format!("Fetching {}", self.school.raw_specifier));
        if let Err(e) = git.fetch("origin", "main") {
            ace.warn(&e.to_string());
            ace.hint(crate::git::auth_hint());
            return Err(PrepareError::Clone(e.to_string()));
        }

        if git
            .is_ahead_of("origin/main")
            .map_err(|e| PrepareError::Clone(e.to_string()))?
        {
            return Ok(PullOutcome::AheadOfOrigin {
                clone_path: clone_path.display().to_string(),
            });
        }

        if let Err(e) = git.merge_ff_only("origin/main") {
            return Ok(PullOutcome::Diverged {
                error: e.to_string(),
            });
        }

        // -- collect skill changes --

        let new_head = git
            .rev_parse("HEAD")
            .map_err(|e| PrepareError::Clone(e.to_string()))?;

        let changes = diff_skill_changes(&git, &old_head, &new_head);

        warn_unusable_skill_folders(ace, clone_path);
        Ok(PullOutcome::Updated { changes })
    }
}

/// Warn about any folder under `skills/` that doesn't resolve to a usable skill
/// — no `SKILL.md`, or an unresolved submodule left by an upstream import bug.
/// A consumer can't fix the school it pulls from, so this informs rather than
/// prescribes a fix.
fn warn_unusable_skill_folders(ace: &mut Ace, school_root: &Path) {
    let skills_dir = school_root.join("skills");
    let Ok(entries) = std::fs::read_dir(&skills_dir) else {
        return;
    };
    let (skills, _) = crate::skills::discover::discover_skills(school_root).unwrap_or_default();

    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        // Skip tier containers (`.curated`/`.experimental`/`.system`) and stray
        // dotfiles — discovery accounts for tier dirs via their own entries.
        if name.starts_with('.') || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }

        let dir = entry.path();
        let usable = skills.iter().any(|s| s.path.starts_with(&dir));
        if !usable {
            ace.warn(&format!(
                "skills/{name}: not a usable skill folder (no SKILL.md) — \
                 likely an unresolved import in the school you're consuming"
            ));
        }
    }
}

fn diff_skill_changes(git: &crate::git::Git<'_>, old: &str, new: &str) -> Vec<SkillChange> {
    if old == new {
        return Vec::new();
    }

    match git.diff_name_status(old, new, Some("skills/")) {
        Ok(stdout) => parse_diff_output(&stdout),
        Err(_) => Vec::new(),
    }
}

/// Check if the last fetch was longer ago than FETCH_COOLDOWN.
/// Returns true (stale) if FETCH_HEAD is missing or old.
fn is_stale(clone_path: &Path) -> bool {
    let fetch_head = clone_path.join(".git/FETCH_HEAD");
    let age = fetch_head
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok());

    match age {
        Some(d) => d > FETCH_COOLDOWN,
        None => true,
    }
}

fn parse_diff_output(output: &str) -> Vec<SkillChange> {
    let mut seen = HashSet::new();
    let mut changes = Vec::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (status, path) = match line.split_once('\t') {
            Some(pair) => pair,
            None => continue,
        };

        let Some(name) = skill_name_from_path(path) else {
            continue;
        };

        if !seen.insert(name.to_string()) {
            continue;
        }

        let kind = match status.chars().next() {
            Some('A') => ChangeKind::Added,
            Some('D') => ChangeKind::Removed,
            _ => ChangeKind::Modified,
        };

        changes.push(SkillChange {
            name: name.to_string(),
            kind,
        });
    }

    changes
}

/// Extract the skill identity from a diff path under the school's
/// `skills/` tree.
///
/// School storage lands a skill at `<school>/skills/<identity>/SKILL.md`
/// where `<identity>` is the post-strip path from
/// `docs/spec/skills/model.md` — flat (`foo`) or nested
/// (`typescript/coding`). Tier subdirs (`.curated/`, `.experimental/`,
/// `.system/`) are recognized as discovery prefixes too, since older
/// schools may still hold imports laid out under them.
///
/// Returns the identity as the portion between `skills/` (after an
/// optional tier prefix) and the path component immediately preceding
/// `SKILL.md` (or the file's parent for non-`SKILL.md` entries inside
/// a skill dir). `None` for paths outside `skills/` or paths that don't
/// resolve to a skill body.
fn skill_name_from_path(path: &str) -> Option<&str> {
    let rest = path.strip_prefix("skills/")?;
    let first_slash = rest.find('/')?;
    let first = &rest[..first_slash];
    let body = if crate::skills::discover::TIER_DIRS.contains(&first) {
        &rest[first_slash + 1..]
    } else {
        rest
    };

    // Walk path components until the one just above `SKILL.md`. For other
    // files inside the skill dir we still pull out the same parent —
    // the imports resolver / discovery layer already keyed the identity
    // by parent dir.
    if let Some(pos) = body.rfind("/SKILL.md") {
        let identity = &body[..pos];
        if identity.is_empty() {
            return None;
        }
        return Some(identity);
    }
    // Non-SKILL.md path under a skill — strip the trailing file component.
    let last_slash = body.rfind('/')?;
    let identity = &body[..last_slash];
    if identity.is_empty() {
        return None;
    }
    Some(identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_added_modified_removed() {
        let output = "A\tskills/new-skill/SKILL.md\n\
                       M\tskills/existing/SKILL.md\n\
                       D\tskills/old-skill/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 3);
        assert_eq!(changes[0].name, "new-skill");
        assert_eq!(changes[0].kind, ChangeKind::Added);
        assert_eq!(changes[1].name, "existing");
        assert_eq!(changes[1].kind, ChangeKind::Modified);
        assert_eq!(changes[2].name, "old-skill");
        assert_eq!(changes[2].kind, ChangeKind::Removed);
    }

    #[test]
    fn dedup_by_skill_name() {
        let output = "M\tskills/my-skill/SKILL.md\n\
                       M\tskills/my-skill/prompt.md\n\
                       A\tskills/other/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].name, "my-skill");
        assert_eq!(changes[1].name, "other");
    }

    #[test]
    fn ignores_non_skills_paths() {
        let output = "M\tREADME.md\n\
                       M\tschool.toml\n\
                       A\tskills/real/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "real");
    }

    #[test]
    fn empty_output() {
        assert!(parse_diff_output("").is_empty());
        assert!(parse_diff_output("  \n  \n").is_empty());
    }

    #[test]
    fn extracts_skill_name_under_curated_tier() {
        let output = "M\tskills/.curated/foo/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "foo");
    }

    #[test]
    fn extracts_skill_name_under_experimental_tier() {
        let output = "A\tskills/.experimental/bar/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "bar");
        assert_eq!(changes[0].kind, ChangeKind::Added);
    }

    #[test]
    fn extracts_skill_name_under_system_tier() {
        let output = "D\tskills/.system/baz/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "baz");
        assert_eq!(changes[0].kind, ChangeKind::Removed);
    }

    #[test]
    fn dedup_within_tier_dir() {
        let output = "M\tskills/.curated/foo/SKILL.md\n\
                       M\tskills/.curated/foo/notes.md\n\
                       M\tskills/.curated/other/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].name, "foo");
        assert_eq!(changes[1].name, "other");
    }

    #[test]
    fn tier_dir_alone_is_skipped() {
        // Just the tier dir with no skill name shouldn't crash or appear.
        let output = "M\tskills/.curated/\n";
        let changes = parse_diff_output(output);
        assert!(changes.is_empty());
    }

    #[test]
    fn rename_treated_as_modified() {
        let output = "R100\tskills/old-name/SKILL.md\tskills/new-name/SKILL.md\n";
        let changes = parse_diff_output(output);
        // R lines have the tab-separated old path first; parse picks up old-name as Modified
        assert!(!changes.is_empty());
    }

    // -- nested identity (spec: skills/model.md) --

    #[test]
    fn nested_identity_skill_md_extracted() {
        let output = "A\tskills/typescript/coding/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "typescript/coding");
        assert_eq!(changes[0].kind, ChangeKind::Added);
    }

    #[test]
    fn nested_identity_inner_file_extracted() {
        // Modification to a non-SKILL.md file inside the skill dir still
        // bubbles up to the skill identity for dedup.
        let output = "M\tskills/typescript/coding/notes.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "typescript/coding");
    }

    #[test]
    fn nested_dedup_across_inner_files() {
        let output = "M\tskills/typescript/coding/SKILL.md\n\
                       M\tskills/typescript/coding/notes.md\n\
                       A\tskills/python/coding/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 2);
        let mut names: Vec<&str> = changes.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, vec!["python/coding", "typescript/coding"]);
    }

    #[test]
    fn nested_under_tier_dir() {
        // Legacy support: school with `skills/.curated/group/leaf/SKILL.md`.
        let output = "M\tskills/.curated/group/leaf/SKILL.md\n";
        let changes = parse_diff_output(output);
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "group/leaf");
    }
}
