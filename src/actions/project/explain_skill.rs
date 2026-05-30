//! Detailed explanation of a single skill's resolution + provenance.
//!
//! Pure: takes `&Skills<Decided>` and a target name; either returns a
//! reference to the matching `Skill<Decided>` (caller renders) or an
//! `ExplainError::NotFound` carrying near-match suggestions.

use crate::skills::{Decided, Entry, Skill, Skills, Source};

#[derive(Debug, PartialEq, Eq)]
pub enum ExplainError {
    NotFound { name: String, near: Vec<String> },
}

impl std::fmt::Display for ExplainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExplainError::NotFound { name, near } => write!(
                f,
                "unknown skill `{}`{}",
                crate::skills::name::render(name),
                format_near(near),
            ),
        }
    }
}

impl std::error::Error for ExplainError {}

fn format_near(near: &[String]) -> String {
    if near.is_empty() {
        String::new()
    } else {
        let rendered: Vec<String> = near
            .iter()
            .map(|name| crate::skills::name::render(name).to_string())
            .collect();
        format!(" — did you mean: {}", rendered.join(", "))
    }
}

pub fn find_or_suggest<'a>(
    skills: &'a Skills<Decided>,
    name: &str,
) -> Result<&'a Skill<Decided>, ExplainError> {
    if let Some(skill) = skills.find(name) {
        return Ok(skill);
    }
    Err(ExplainError::NotFound {
        name: name.to_string(),
        near: near_matches(name, skills),
    })
}

/// Render a single resolved skill's explanation block.
pub fn render(skill: &Skill<Decided>) -> String {
    let mut s = format!(
        "{} ({})\n  status: {}\n  trace:\n",
        crate::skills::name::render(&skill.name),
        skill.tier.label(),
        skill.state.decision.label(),
    );
    for entry in &skill.state.trace {
        s.push_str("    ");
        s.push_str(&format_trace_line(entry));
        s.push('\n');
    }
    s
}

fn format_trace_line(e: &Entry) -> String {
    // The synthetic "no-filter" base is reported as `Source::Default`; render
    // it as "implicit" for user-facing continuity with the prior label.
    let source_label = match e.source {
        Source::Default => "implicit",
        s => s.label(),
    };
    format!(
        "{:>9}  {}: {} \"{}\"",
        e.op.label(),
        source_label,
        e.field.label(),
        crate::skills::name::render(&e.pattern),
    )
}

/// Up to 5 names with at least 3-char overlap with the query. Cheap heuristic;
/// good enough for "did you mean" prompts without pulling in a fuzzy crate.
fn near_matches(query: &str, skills: &Skills<Decided>) -> Vec<String> {
    let q = query.to_lowercase();
    let mut scored: Vec<(usize, &str)> = skills
        .iter()
        .filter_map(|s| {
            let name = s.name.as_str();
            let lower = name.to_lowercase();
            let score = overlap_score(&q, &lower);
            (score >= 3).then_some((score, name))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    scored
        .into_iter()
        .take(5)
        .map(|(_, n)| n.to_string())
        .collect()
}

/// Length of longest contiguous substring of `q` appearing in `name`.
fn overlap_score(q: &str, name: &str) -> usize {
    let qb = q.as_bytes();
    let nb = name.as_bytes();
    if qb.is_empty() || nb.is_empty() {
        return 0;
    }
    let mut best = 0;
    for start in 0..qb.len() {
        for end in (start + 1..=qb.len()).rev() {
            if end - start <= best {
                break;
            }
            let sub = &q[start..end];
            if name.contains(sub) {
                best = end - start;
                break;
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ace_toml::AceToml;
    use crate::config::tree::Tree;
    use crate::skills::discover::{DiscoveredSkill, Tier};
    use crate::skills::{Decision, Discovered};
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
            id: crate::skills::SkillId::from_basename(name),
            path: PathBuf::from(format!("/school/{name}")),
            tier,
            internal: false,
            frontmatter_name: None,
        }
    }

    fn resolve(names: &[&str], t: &Tree) -> Skills<Decided> {
        let disc: Vec<DiscoveredSkill> =
            names.iter().map(|n| discovered(n, Tier::Curated)).collect();
        Skills::<Discovered>::from_discovered(&disc).resolve(t)
    }

    #[test]
    fn finds_active_skill_with_base_only() {
        let s = resolve(
            &["a", "rust-coding"],
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let skill = find_or_suggest(&s, "rust-coding").expect("known");
        assert_eq!(skill.name, "rust-coding");
        assert_eq!(skill.state.decision, Decision::Included);
        assert_eq!(skill.tier, Tier::Curated);
        assert_eq!(skill.state.trace.len(), 1);

        let out = render(skill);
        assert!(out.contains("active"));
        assert!(out.contains("base"));
        assert!(out.contains("implicit"));
    }

    #[test]
    fn renders_excluded_skill() {
        let s = resolve(
            &["rust-fmt"],
            &tree(
                AceToml::default(),
                ace(&[], &[], &["rust-fmt"]),
                AceToml::default(),
            ),
        );
        let skill = find_or_suggest(&s, "rust-fmt").expect("known");
        assert_eq!(skill.state.decision, Decision::Excluded);
        let out = render(skill);
        assert!(out.contains("excluded"));
        assert!(out.contains("removed"));
        assert!(out.contains("project"));
        assert!(out.contains("exclude_skills"));
    }

    #[test]
    fn renders_readded_full_chain() {
        let s = resolve(
            &["rust-fmt", "rust-coding"],
            &tree(
                ace(&[], &["rust-fmt"], &[]),
                ace(&["rust-*"], &[], &["rust-fmt"]),
                AceToml::default(),
            ),
        );
        let skill = find_or_suggest(&s, "rust-fmt").expect("known");
        assert_eq!(skill.state.decision, Decision::Included);
        assert_eq!(skill.state.trace.len(), 3);
        let out = render(skill);
        assert!(out.contains("base"));
        assert!(out.contains("removed"));
        assert!(out.contains("re-added"));
    }

    #[test]
    fn unknown_skill_returns_not_found_with_near() {
        let s = resolve(
            &["a", "b", "rust-coding", "rust-fmt"],
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let err = find_or_suggest(&s, "rust-cod").unwrap_err();
        match err {
            ExplainError::NotFound { name, near } => {
                assert_eq!(name, "rust-cod");
                assert!(near.contains(&"rust-coding".to_string()), "got: {near:?}");
            }
        }
    }

    #[test]
    fn unknown_skill_with_no_overlap_returns_empty_near() {
        let s = resolve(
            &["rust-coding"],
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let err = find_or_suggest(&s, "xz").unwrap_err();
        match err {
            ExplainError::NotFound { near, .. } => assert!(near.is_empty(), "got: {near:?}"),
        }
    }

    #[test]
    fn near_matches_capped_at_five() {
        let many: Vec<String> = (0..20).map(|i| format!("skill-{i:02}")).collect();
        let many_refs: Vec<&str> = many.iter().map(|s| s.as_str()).collect();
        let s = resolve(
            &many_refs,
            &tree(AceToml::default(), AceToml::default(), AceToml::default()),
        );
        let err = find_or_suggest(&s, "skill-").unwrap_err();
        match err {
            ExplainError::NotFound { near, .. } => {
                assert_eq!(near.len(), 5, "should cap near matches; got {}", near.len());
            }
        }
    }
}
