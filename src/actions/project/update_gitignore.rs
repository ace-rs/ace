use std::collections::BTreeSet;
use std::path::Path;

use crate::ace::Ace;
use crate::actions::project::SCHOOL_FOLDERS;
use crate::backend::Kind;

const MARKER_START: &str = "# ACE-managed — do not edit this block.";
const MARKER_END: &str = "# end ACE";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Consumer project: includes the dynamic per-folder section.
    Project,
    /// School repo: static prelude only.
    School,
}

pub struct UpdateGitignore<'a> {
    pub project_dir: &'a Path,
    pub scope: Scope,
}

impl UpdateGitignore<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), std::io::Error> {
        let path = self.project_dir.join(".gitignore");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        let block = build_block(self.scope);

        let new_content = if existing.contains(MARKER_START) {
            replace_block(&existing, &block)
        } else {
            append_block(&existing, &block)
        };

        if new_content == existing {
            return Ok(());
        }

        std::fs::write(&path, &new_content)?;
        ace.done("Updated .gitignore with ACE patterns");
        Ok(())
    }
}

/// Static prelude — universal OS / editor cruft, applies to every ACE-managed
/// `.gitignore` regardless of scope. Kept inside the marker block so future
/// updates propagate; explicitly excludes `.env*` and language-specific
/// patterns (see `docs/decisions/2026-05-10-gitignore-managed-block.md`).
const STATIC_PRELUDE: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
    "*.swp",
    "*.swo",
    "*~",
    ".vscode/",
    ".idea/",
];

fn build_block(scope: Scope) -> String {
    let mut lines = vec![
        MARKER_START.to_string(),
        "# Managed by `ace setup` / `ace school init`. See https://github.com/ace-rs/ace".to_string(),
    ];

    for line in STATIC_PRELUDE {
        lines.push((*line).to_string());
    }

    if scope == Scope::Project {
        let dirs: BTreeSet<&str> = Kind::ALL.iter().map(|b| b.backend_dir()).collect();
        let folders: BTreeSet<&str> = SCHOOL_FOLDERS.iter().copied().collect();
        for dir in &dirs {
            for folder in &folders {
                lines.push(format!("{dir}/{folder}"));
            }
        }
        lines.push("ace.local.toml".to_string());
    }

    lines.push(MARKER_END.to_string());
    lines.push(String::new()); // trailing newline
    lines.join("\n")
}

fn replace_block(content: &str, block: &str) -> String {
    let Some(start) = content.find(MARKER_START) else {
        return content.to_string();
    };
    let search_from = start + MARKER_START.len();
    let Some(end_marker) = content[search_from..].find(MARKER_END) else {
        return content.to_string();
    };
    let end = search_from + end_marker + MARKER_END.len();

    // Skip trailing newline after end marker.
    let end = if content[end..].starts_with('\n') { end + 1 } else { end };

    let mut result = content[..start].to_string();
    result.push_str(block);
    result.push_str(&content[end..]);
    result
}

fn append_block(content: &str, block: &str) -> String {
    if content.is_empty() {
        return block.to_string();
    }

    let mut result = content.trim_end().to_string();
    result.push_str("\n\n");
    result.push_str(block);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_block_contains_all_backends_and_local_toml() {
        let block = build_block(Scope::Project);
        assert!(block.contains(".claude/skills"));
        assert!(block.contains(".claude/rules"));
        assert!(block.contains(".claude/commands"));
        assert!(block.contains(".claude/agents"));
        assert!(block.contains(".agents/skills"));
        assert!(block.contains(".agents/agents"));
        assert!(block.contains("ace.local.toml"));
        assert!(block.contains(MARKER_START));
        assert!(block.contains(MARKER_END));
    }

    #[test]
    fn both_scopes_contain_static_prelude() {
        for scope in [Scope::Project, Scope::School] {
            let block = build_block(scope);
            for line in STATIC_PRELUDE {
                assert!(block.contains(line), "scope {scope:?} missing {line}");
            }
        }
    }

    #[test]
    fn neither_scope_contains_dropped_patterns() {
        for scope in [Scope::Project, Scope::School] {
            let block = build_block(scope);
            assert!(!block.contains(".env"), "{scope:?} should not include .env");
            assert!(!block.contains("__pycache__"), "{scope:?} should not include Python");
            assert!(!block.contains("*.pyc"), "{scope:?} should not include *.pyc");
        }
    }

    #[test]
    fn school_block_omits_dynamic_section() {
        let block = build_block(Scope::School);
        assert!(!block.contains(".claude/skills"));
        assert!(!block.contains(".agents/skills"));
        assert!(!block.contains("ace.local.toml"));
    }

    #[test]
    fn append_to_empty() {
        let block = build_block(Scope::Project);
        let result = append_block("", &block);
        assert_eq!(result, block);
    }

    #[test]
    fn append_to_existing() {
        let block = build_block(Scope::Project);
        let result = append_block("node_modules/\n", &block);
        assert!(result.starts_with("node_modules/"));
        assert!(result.contains(MARKER_START));
        assert!(result.ends_with('\n'));
    }

    #[test]
    fn append_adds_blank_line_separator() {
        let block = build_block(Scope::Project);
        let result = append_block("node_modules/\n", &block);
        assert!(result.contains("node_modules/\n\n#"));
    }

    #[test]
    fn replace_existing_block() {
        let original = format!(
            "node_modules/\n{MARKER_START}\n.old/skills/\n{MARKER_END}\n.env\n"
        );
        let block = build_block(Scope::Project);
        let result = replace_block(&original, &block);

        assert!(result.contains("node_modules/"));
        assert!(result.contains(".env"));
        assert!(result.contains(".claude/skills"));
        assert!(!result.contains(".old/skills/"));
    }

    #[test]
    fn replace_preserves_surrounding_content() {
        let original = format!(
            "before\n{MARKER_START}\nold stuff\n{MARKER_END}\nafter\n"
        );
        let block = build_block(Scope::Project);
        let result = replace_block(&original, &block);

        assert!(result.contains("before\n"));
        assert!(result.contains("after\n"));
    }

    #[test]
    fn block_dirs_alphabetically_sorted() {
        let block = build_block(Scope::Project);
        let agents_pos = block.find(".agents/skills").expect(".agents/skills present");
        let claude_pos = block.find(".claude/skills").expect(".claude/skills present");
        assert!(
            agents_pos < claude_pos,
            ".agents/* should appear before .claude/* in gitignore block"
        );
    }

    #[test]
    fn block_folders_alphabetically_sorted_within_dir() {
        let block = build_block(Scope::Project);
        let agents = block.find(".claude/agents").expect(".claude/agents present");
        let commands = block.find(".claude/commands").expect(".claude/commands present");
        let rules = block.find(".claude/rules").expect(".claude/rules present");
        let skills = block.find(".claude/skills").expect(".claude/skills present");
        assert!(agents < commands, "agents < commands");
        assert!(commands < rules, "commands < rules");
        assert!(rules < skills, "rules < skills");
    }

    #[test]
    fn idempotent_when_unchanged() {
        let block = build_block(Scope::Project);
        let content = append_block("", &block);
        let replaced = replace_block(&content, &block);

        assert!(replaced.contains(MARKER_START));
        assert!(replaced.contains(".claude/skills"));
    }
}
