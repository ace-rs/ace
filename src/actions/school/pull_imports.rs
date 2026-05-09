use std::collections::HashMap;
use std::path::Path;

use crate::ace::Ace;
use crate::config;
use crate::glob;
use crate::skills::discover::{DiscoveredSkill, Tier, discover_skills};
use crate::skills::{Discovered, Skills};

pub struct PullImports<'a> {
    pub school_root: &'a Path,
}

#[derive(Debug, thiserror::Error)]
pub enum PullImportsError {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Config(#[from] config::ConfigError),
    #[error("{0}")]
    Git(#[from] crate::git::GitError),
}

pub enum PullImportsResult {
    NoImports,
    Updated {
        #[allow(dead_code)] // part of result API
        count: usize,
    },
}

impl PullImports<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<PullImportsResult, PullImportsError> {
        let toml_path = self.school_root.join("school.toml");
        let school = config::school_toml::load(&toml_path)?;

        if school.imports.is_empty() {
            return Ok(PullImportsResult::NoImports);
        }

        let by_source = group_by_source(&school.imports);
        let skills_dir = self.school_root.join("skills");

        // Discover each source once. Multiple decls against the same source
        // share the cached clone + discovery rather than re-walking per decl.
        let mut discovery: HashMap<&str, Vec<DiscoveredSkill>> = HashMap::new();
        for (source, _) in &by_source {
            ace.progress(&format!("Fetching {source}"));
            let cached = match crate::git::ensure_source_cache(source) {
                Ok(p) => p,
                Err(e) => {
                    ace.warn(&e.to_string());
                    ace.hint(crate::git::auth_hint());
                    return Err(e.into());
                }
            };
            discovery.insert(source, discover_skills(&cached)?);
        }

        // Single pass in declaration order, last-wins on collision. See
        // docs/spec/skills-sync.md § Import Merge Strategy.
        let mut accumulator: Skills<Discovered> = Skills::default();
        for (source, decls) in &by_source {
            let discovered = &discovery[source];
            let full = Skills::<Discovered>::from_discovered_with_source(discovered, source);

            let mut names: Vec<String> = Vec::new();
            for decl in decls {
                let resolved = resolve_import_names(&full, decl);
                if resolved.is_empty() {
                    ace.warn(&format!("no skills matching {} in {source}", decl.skill));
                    continue;
                }
                for n in resolved {
                    if !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            if names.is_empty() {
                continue;
            }

            let batch_discovered: Vec<DiscoveredSkill> = discovered
                .iter()
                .filter(|d| names.iter().any(|n| n == &d.name))
                .cloned()
                .collect();
            let batch = Skills::<Discovered>::from_discovered_with_source(
                &batch_discovered,
                source,
            );
            accumulator.merge(batch);
        }

        let winning_names: Vec<String> = accumulator.names().map(String::from).collect();
        let name_refs: Vec<&str> = winning_names.iter().map(String::as_str).collect();
        let changes = accumulator.copy_into(&skills_dir, &name_refs)?;

        let count = changes.len();
        ace.done(&crate::skills::format_pull_summary(&changes));
        Ok(PullImportsResult::Updated { count })
    }
}


/// Resolve the list of skill names to copy for an import entry given a
/// discovered set from the source repo. Explicit names are looked up
/// across all tiers; glob patterns are tier-gated.
fn resolve_import_names(
    set: &Skills<Discovered>,
    decl: &config::school_toml::ImportDecl,
) -> Vec<String> {
    if glob::is_glob(&decl.skill) {
        let mut allowed = vec![Tier::Curated];
        if decl.include_experimental {
            allowed.push(Tier::Experimental);
        }
        if decl.include_system {
            allowed.push(Tier::System);
        }
        let filtered = set.filter_tiers(&allowed);
        filtered.matching(&decl.skill)
            .into_iter()
            .map(String::from)
            .collect()
    } else if set.names().any(|n| n == decl.skill) {
        vec![decl.skill.clone()]
    } else {
        Vec::new()
    }
}

/// Group decls by source preserving school.toml encounter order. Two sources
/// colliding on the same skill within a single pass resolve in declaration
/// order — first-declared wins — which would not be deterministic with a
/// `HashMap`.
fn group_by_source(
    imports: &[config::school_toml::ImportDecl],
) -> Vec<(&str, Vec<&config::school_toml::ImportDecl>)> {
    let mut order: Vec<&str> = Vec::new();
    let mut by_source: HashMap<&str, Vec<&config::school_toml::ImportDecl>> = HashMap::new();
    for imp in imports {
        let key = imp.source.as_str();
        if !by_source.contains_key(key) {
            order.push(key);
        }
        by_source.entry(key).or_default().push(imp);
    }
    order
        .into_iter()
        .map(|s| (s, by_source.remove(s).expect("seeded above")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::school_toml::ImportDecl;
    use crate::skills::discover::DiscoveredSkill;

    fn discovered(name: &str, tier: Tier) -> DiscoveredSkill {
        DiscoveredSkill {
            name: name.to_string(),
            path: std::path::PathBuf::from(name),
            tier,
        }
    }

    fn import(skill: &str, experimental: bool, system: bool) -> ImportDecl {
        ImportDecl {
            skill: skill.to_string(),
            source: "owner/repo".to_string(),
            include_experimental: experimental,
            include_system: system,
        }
    }

    #[test]
    fn resolve_glob_matches_curated_by_default() {
        let set = Skills::<Discovered>::from_discovered(&[
            discovered("alpha", Tier::Curated),
            discovered("beta",  Tier::Experimental),
            discovered("gamma", Tier::System),
        ]);
        let names = resolve_import_names(&set, &import("*", false, false));
        assert_eq!(names, vec!["alpha".to_string()]);
    }

    #[test]
    fn resolve_glob_with_experimental_flag_adds_that_tier() {
        let set = Skills::<Discovered>::from_discovered(&[
            discovered("alpha", Tier::Curated),
            discovered("beta",  Tier::Experimental),
            discovered("gamma", Tier::System),
        ]);
        let mut names = resolve_import_names(&set, &import("*", true, false));
        names.sort();
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn resolve_glob_with_both_flags_adds_all_tiers() {
        let set = Skills::<Discovered>::from_discovered(&[
            discovered("alpha", Tier::Curated),
            discovered("beta",  Tier::Experimental),
            discovered("gamma", Tier::System),
        ]);
        let mut names = resolve_import_names(&set, &import("*", true, true));
        names.sort();
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string(), "gamma".to_string()]);
    }

    #[test]
    fn resolve_explicit_name_finds_skill_in_any_tier() {
        let set = Skills::<Discovered>::from_discovered(&[
            discovered("shell", Tier::Experimental),
        ]);
        let names = resolve_import_names(&set, &import("shell", false, false));
        assert_eq!(names, vec!["shell".to_string()]);
    }

    #[test]
    fn resolve_explicit_name_finds_skill_in_system_tier() {
        let set = Skills::<Discovered>::from_discovered(&[
            discovered("skill-creator", Tier::System),
        ]);
        let names = resolve_import_names(&set, &import("skill-creator", false, false));
        assert_eq!(names, vec!["skill-creator".to_string()]);
    }

    #[test]
    fn resolve_explicit_name_missing_returns_empty() {
        let set = Skills::<Discovered>::from_discovered(&[
            discovered("alpha", Tier::Curated),
        ]);
        let names = resolve_import_names(&set, &import("missing", false, false));
        assert!(names.is_empty());
    }

    #[test]
    fn resolve_glob_no_matches_returns_empty() {
        let set = Skills::<Discovered>::from_discovered(&[
            discovered("alpha", Tier::Experimental),
        ]);
        let names = resolve_import_names(&set, &import("*", false, false));
        assert!(names.is_empty(), "curated-only default should not match experimental");
    }
}
