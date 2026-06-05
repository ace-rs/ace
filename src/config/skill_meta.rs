use std::fs;
use std::path::Path;

/// Metadata extracted from a SKILL.md YAML frontmatter.
#[derive(Debug)]
pub struct SkillMeta {
    pub name: String,
    pub description: String,
}

/// Read and parse SKILL.md frontmatter from a skill directory.
/// Returns None if SKILL.md is missing or has no valid frontmatter.
pub fn load(skill_dir: &Path) -> Option<SkillMeta> {
    let content = fs::read_to_string(skill_dir.join("SKILL.md")).ok()?;
    parse(&content)
}

/// Parse YAML frontmatter from SKILL.md content.
/// Expects `---` delimited frontmatter with `name` and `description` fields.
fn parse(content: &str) -> Option<SkillMeta> {
    let content = content.trim_start();
    if !content.starts_with("---") {
        return None;
    }

    let after_open = &content[3..];
    let close = after_open.find("\n---")?;
    let block = &after_open[..close];

    let mut name = None;
    let mut desc_parts: Vec<String> = Vec::new();
    let mut in_desc = false;

    for line in block.lines() {
        let trimmed = line.trim();

        // New key starts (not indented, contains `:`)
        if !line.starts_with(' ') && !line.starts_with('\t') && line.contains(':') {
            in_desc = false;

            if let Some(val) = trimmed.strip_prefix("name:") {
                name = Some(val.trim().to_string());
            } else if let Some(val) = trimmed.strip_prefix("description:") {
                let val = val.trim();
                if val == ">" || val == "|" {
                    // Block scalar — collect continuation lines
                    in_desc = true;
                } else {
                    desc_parts.push(val.to_string());
                }
            }
        } else if in_desc && (line.starts_with(' ') || line.starts_with('\t')) {
            desc_parts.push(trimmed.to_string());
        }
    }

    let name = name.filter(|n| !n.is_empty())?;
    let description = desc_parts.join(" ");

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
