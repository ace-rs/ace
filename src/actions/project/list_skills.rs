//! Render the `ace skills` listing.
//!
//! Walks `Skills<Decided>` directly — no intermediate row struct. Default
//! hides excluded; `show_excluded` flips that. Output sorted by name (the
//! resolver emits skills in BTreeMap order, so the iterator is already sorted).

use std::fmt::Write;

use crate::skills::{Decided, Entry, Skill, Skills, Source};

/// Tab-separated table with header. Matches `ace paths` style for machine parsing.
pub fn render_table(skills: &Skills<Decided>, show_excluded: bool) -> String {
    let mut out = String::from("NAME\tTIER\tSTATUS\tREASON\n");
    for skill in visible(skills, show_excluded) {
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}",
            crate::skills::name::render(&skill.name),
            skill.tier.label(),
            skill.status().label(),
            reason_for(skill),
        );
    }
    out
}

/// Bare names, one per line. Scriptable.
pub fn render_names(skills: &Skills<Decided>, show_excluded: bool) -> String {
    let mut out = String::new();
    for skill in visible(skills, show_excluded) {
        out.push_str(crate::skills::name::render(&skill.name).as_ref());
        out.push('\n');
    }
    out
}

fn visible(skills: &Skills<Decided>, show_excluded: bool) -> impl Iterator<Item = &Skill<Decided>> {
    // Default view shows only admissible, selected skills. `show_excluded`
    // widens to everything else — both config-excluded and name-rejected.
    skills
        .iter()
        .filter(move |s| show_excluded || s.status() == crate::skills::Status::Active)
}

/// Human-readable summary of the last trace contribution. Used in the REASON
/// column. `Implicit` gets a special-case phrasing; everything else reads as
/// `<scope>: <field> "<pattern>"`.
fn reason_for(skill: &Skill<Decided>) -> String {
    let Some(last) = skill.state.trace.last() else {
        return String::new();
    };
    format_reason(last)
}

fn format_reason(e: &Entry) -> String {
    match e.source {
        Source::Default => "implicit base (no skills filter)".to_string(),
        _ => format!(
            "{}: {} \"{}\"",
            e.source.label(),
            e.field.label(),
            crate::skills::name::render(&e.pattern),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ace_toml::AceToml;
    use crate::config::tree::Tree;
    use crate::skills::discover::{DiscoveredSkill, Tier};
    use crate::skills::Discovered;
    use std::path::PathBuf;

    fn ace(skills: &[&str], inc: &[&str], exc: &[&str]) -> AceToml {
        AceToml {
            skills: skills.iter().map(|s| s.to_string()).collect(),
            include_skills: inc.iter().map(|s| s.to_string()).collect(),
            exclude_skills: exc.iter().map(|s| s.to_string()).collect(),
            ..AceToml::default()
        }
    }

    fn tree(user: AceToml, project: AceToml, local: AceToml) -> Tree {
        Tree {
            user: Some(user),
            project: Some(project),
            local: Some(local),
            school: None,
        }
    }

    fn discovered(name: &str, tier: Tier) -> DiscoveredSkill {
        DiscoveredSkill {
            locator: crate::skills::Locator::from_basename(name),
            path: PathBuf::from(format!("/school/{name}")),
            tier,
            internal: false,
            frontmatter_name: None,
        }
    }

    fn all_curated(names: &[&str]) -> Vec<DiscoveredSkill> {
        names.iter().map(|n| discovered(n, Tier::Curated)).collect()
    }

    fn resolve(disc: Vec<DiscoveredSkill>, tree: &Tree) -> Skills<Decided> {
        Skills::<Discovered>::from_discovered(&disc).resolve(tree)
    }

    #[test]
    fn default_hides_excluded() {
        let s = resolve(
            all_curated(&["a", "b", "rust-coding", "rust-fmt"]),
            &tree(
                AceToml::default(),
                ace(&["rust-*"], &[], &[]),
                AceToml::default(),
            ),
        );
        let out = render_table(&s, false);
        let lines: Vec<&str> = out.lines().skip(1).collect();
        let names: Vec<&str> = lines
            .iter()
            .map(|l| l.split('\t').next().unwrap())
            .collect();
        assert_eq!(names, vec!["rust-coding", "rust-fmt"]);
        assert!(lines.iter().all(|l| l.contains("\tactive\t")));
    }

    #[test]
    fn show_all_includes_excluded() {
        let s = resolve(
            all_curated(&["a", "b", "rust-coding", "rust-fmt"]),
            &tree(
                AceToml::default(),
                ace(&["rust-*"], &[], &[]),
                AceToml::default(),
            ),
        );
        let out = render_table(&s, true);
        let data_lines: Vec<&str> = out.lines().skip(1).collect();
        assert_eq!(data_lines.len(), 4);
        let excluded: Vec<&str> = data_lines
            .iter()
            .filter(|l| l.contains("\texcluded\t"))
            .map(|l| l.split('\t').next().unwrap())
            .collect();
        assert_eq!(excluded, vec!["a", "b"]);
    }

    #[test]
    fn rows_sorted_by_name() {
        let s = resolve(
            all_curated(&["rust-fmt", "a", "rust-coding", "b"]),
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let out = render_names(&s, false);
        let lines: Vec<&str> = out.lines().collect();
        let mut sorted = lines.clone();
        sorted.sort();
        assert_eq!(lines, sorted);
    }

    #[test]
    fn reason_picks_last_contribution() {
        let s = resolve(
            all_curated(&["a", "rust-coding", "rust-fmt"]),
            &tree(
                ace(&[], &["rust-*"], &[]),
                ace(&["a"], &[], &[]),
                AceToml::default(),
            ),
        );
        let out = render_table(&s, false);
        let line = out
            .lines()
            .find(|l| l.starts_with("rust-coding\t"))
            .expect("rc");
        assert!(line.contains("user"));
        assert!(line.contains("include_skills"));
        assert!(line.contains("rust-*"));
    }

    #[test]
    fn implicit_base_reason_when_all_empty() {
        let s = resolve(
            all_curated(&["a"]),
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let out = render_table(&s, false);
        let line = out.lines().find(|l| l.starts_with("a\t")).expect("a");
        assert!(line.contains("implicit"));
    }

    #[test]
    fn tier_passthrough() {
        let disc = vec![
            discovered("a", Tier::Curated),
            discovered("b", Tier::Experimental),
            discovered("c", Tier::System),
        ];
        let s = resolve(
            disc,
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let out = render_table(&s, false);
        let tier_for = |name: &str| -> String {
            out.lines()
                .find(|l| l.starts_with(&format!("{name}\t")))
                .unwrap()
                .split('\t')
                .nth(1)
                .unwrap()
                .to_string()
        };
        assert_eq!(tier_for("a"), "curated");
        assert_eq!(tier_for("b"), "experimental");
        assert_eq!(tier_for("c"), "system");
    }

    #[test]
    fn excluded_row_reports_removal_reason() {
        let s = resolve(
            all_curated(&["a", "b"]),
            &tree(
                AceToml::default(),
                ace(&[], &[], &["a"]),
                AceToml::default(),
            ),
        );
        let out = render_table(&s, true);
        let line = out.lines().find(|l| l.starts_with("a\t")).expect("a");
        assert!(line.contains("\texcluded\t"));
        assert!(line.contains("project"));
        assert!(line.contains("exclude_skills"));
    }

    #[test]
    fn render_table_has_header_and_one_line_per_skill() {
        let s = resolve(
            all_curated(&["a", "b"]),
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let out = render_table(&s, false);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "NAME\tTIER\tSTATUS\tREASON");
        assert_eq!(lines.len(), 3);
        for line in &lines[1..] {
            assert_eq!(line.split('\t').count(), 4, "line: {line:?}");
        }
    }

    #[test]
    fn render_names_one_per_line() {
        let s = resolve(
            all_curated(&["a", "b", "rust-coding"]),
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let out = render_names(&s, false);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines, vec!["a", "b", "rust-coding"]);
    }

    #[test]
    fn render_table_empty_just_header() {
        let s = resolve(
            vec![],
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let out = render_table(&s, false);
        assert_eq!(out, "NAME\tTIER\tSTATUS\tREASON\n");
    }
}
