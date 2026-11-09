use std::fs;
use std::path::Path;

use super::frontmatter;

/// Display metadata from a SKILL.md frontmatter.
#[derive(Debug)]
pub struct SkillMeta {
    pub name: String,
    /// The `description` field as one line: every run of whitespace, line breaks
    /// included, becomes a single space, so a block scalar renders on one listing line.
    pub description: String,
}

/// Read and parse SKILL.md frontmatter from a skill directory.
/// Returns None if SKILL.md is missing or has no valid frontmatter.
pub fn load(skill_dir: &Path) -> Option<SkillMeta> {
    let content = fs::read_to_string(skill_dir.join("SKILL.md")).ok()?;
    parse(&content)
}

/// Parse SKILL.md content. `None` when it has no frontmatter or no non-empty `name`.
fn parse(content: &str) -> Option<SkillMeta> {
    let fields = frontmatter::parse(content)?;
    let name = fields.get("name").filter(|n| !n.is_empty())?.to_string();
    let description = fields
        .get("description")
        .map(|d| d.split_whitespace().collect::<Vec<_>>().join(" "))
        .unwrap_or_default();

    Some(SkillMeta { name, description })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_frontmatter() {
        let content = "---\nname: test-skill\ndescription: A test skill.\n---\n# Body";
        let meta = parse(content).expect("should parse valid frontmatter");
        assert_eq!(meta.name, "test-skill");
        assert_eq!(meta.description, "A test skill.");
    }

    #[test]
    fn block_scalar_description() {
        let content = "\
---
name: multi-line
description: >
  First line of description
  second line of description.
---
# Body";
        let meta = parse(content).expect("should parse block scalar frontmatter");
        assert_eq!(meta.name, "multi-line");
        assert_eq!(
            meta.description,
            "First line of description second line of description."
        );
    }

    #[test]
    fn every_block_scalar_header_yields_one_line_text() {
        // Regression: `>-` used to parse as the literal text ">-".
        for header in [">", ">-", ">+", "|", "|-", "|+", ">2-", "|-2", ">- # note"] {
            let content = format!(
                "---\nname: s\ndescription: {header}\n  First line\n\n  second line.\n---\n"
            );
            let meta = parse(&content).expect("should parse block scalar frontmatter");
            assert_eq!(
                meta.description, "First line second line.",
                "header {header:?}"
            );
        }
    }

    #[test]
    fn missing_frontmatter() {
        assert!(parse("# No frontmatter").is_none());
    }

    #[test]
    fn missing_name() {
        let content = "---\ndescription: no name\n---\n";
        assert!(parse(content).is_none());
    }

    #[test]
    fn empty_name() {
        let content = "---\nname:\ndescription: has desc\n---\n";
        assert!(parse(content).is_none());
    }
}
