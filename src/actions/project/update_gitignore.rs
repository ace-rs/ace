use std::collections::BTreeSet;
use std::path::Path;

use crate::ace::Ace;
use crate::actions::project::SCHOOL_FOLDERS;
use crate::backend::Kind;

const MARKER_START: &str = "# ACE-managed — do not edit this block.";
const MARKER_END: &str = "# end ACE";

/// One-time seed written above the managed block when no `.gitignore` exists.
/// User owns this surface afterwards — ACE never re-syncs it. Convenience for
/// fresh repos, not a contract.
const STATIC_PRELUDE: &[&str] = &[
    ".DS_Store",
    "Thumbs.db",
    "*.swp",
    "*.swo",
    "*~",
    ".vscode/",
    ".idea/",
];

pub struct UpdateGitignore<'a> {
    pub project_dir: &'a Path,
}

impl UpdateGitignore<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), std::io::Error> {
        let path = self.project_dir.join(".gitignore");
        let block = build_block();

        let existing = match std::fs::read_to_string(&path) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };

        let new_content = match &existing {
            Some(s) if s.contains(MARKER_START) => replace_block(s, &block),
            Some(s) => append_block(s, &block),
            None => seed_new_file(&block),
        };

        if existing.as_deref() == Some(new_content.as_str()) {
            return Ok(());
        }

        std::fs::write(&path, &new_content)?;
        ace.done("Updated .gitignore with ACE patterns");
        Ok(())
    }
}

fn build_block() -> String {
    let mut lines = vec![
        MARKER_START.to_string(),
        "# Managed by ACE. See https://github.com/ace-rs/ace".to_string(),
    ];

    let dirs: BTreeSet<&str> = Kind::ALL.iter().map(|b| b.backend_dir()).collect();
    let folders: BTreeSet<&str> = SCHOOL_FOLDERS.iter().copied().collect();
    for dir in &dirs {
        for folder in &folders {
            lines.push(format!("{dir}/{folder}"));
        }
    }
    lines.push("ace.local.toml".to_string());

    lines.push(MARKER_END.to_string());
    lines.push(String::new());
    lines.join("\n")
}

fn seed_new_file(block: &str) -> String {
    let prelude: Vec<String> = STATIC_PRELUDE.iter().map(|s| s.to_string()).collect();
    let mut result = prelude.join("\n");
    result.push_str("\n\n");
    result.push_str(block);
    result
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
    fn block_contains_backends_folders_and_local_toml() {
        let block = build_block();
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
    fn block_omits_static_prelude_and_dropped_patterns() {
        let block = build_block();
        for line in STATIC_PRELUDE {
            assert!(!block.contains(line), "block should not contain prelude line {line}");
        }
        assert!(!block.contains(".env"));
        assert!(!block.contains("__pycache__"));
        assert!(!block.contains("*.pyc"));
    }

    #[test]
    fn seed_new_file_places_prelude_before_block() {
        let block = build_block();
        let result = seed_new_file(&block);
        for line in STATIC_PRELUDE {
            let prelude_pos = result.find(line).expect("prelude line present");
            let block_pos = result.find(MARKER_START).expect("block present");
            assert!(prelude_pos < block_pos, "{line} should precede managed block");
        }
    }

    #[test]
    fn append_to_existing_does_not_seed_prelude() {
        let block = build_block();
        let result = append_block("node_modules/\n", &block);
        for line in STATIC_PRELUDE {
            assert!(!result.contains(line), "append must not inject prelude {line}");
        }
        assert!(result.starts_with("node_modules/"));
        assert!(result.contains(MARKER_START));
    }

    #[test]
    fn append_adds_blank_line_separator() {
        let block = build_block();
        let result = append_block("node_modules/\n", &block);
        assert!(result.contains("node_modules/\n\n#"));
    }

    #[test]
    fn replace_existing_block_preserves_user_content() {
        let original = format!(
            "node_modules/\n{MARKER_START}\n.old/skills/\n{MARKER_END}\n.env\n"
        );
        let block = build_block();
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
        let block = build_block();
        let result = replace_block(&original, &block);

        assert!(result.contains("before\n"));
        assert!(result.contains("after\n"));
    }

    #[test]
    fn block_dirs_alphabetically_sorted() {
        let block = build_block();
        let agents_pos = block.find(".agents/skills").expect(".agents/skills present");
        let claude_pos = block.find(".claude/skills").expect(".claude/skills present");
        assert!(agents_pos < claude_pos);
    }

    #[test]
    fn block_folders_alphabetically_sorted_within_dir() {
        let block = build_block();
        let agents = block.find(".claude/agents").expect(".claude/agents present");
        let commands = block.find(".claude/commands").expect(".claude/commands present");
        let rules = block.find(".claude/rules").expect(".claude/rules present");
        let skills = block.find(".claude/skills").expect(".claude/skills present");
        assert!(agents < commands);
        assert!(commands < rules);
        assert!(rules < skills);
    }
}
