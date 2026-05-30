use std::collections::HashMap;
use std::path::Path;

use crate::ace::Ace;
use crate::config;
use crate::resolver::{DiscoveryBySource, ImportVerdict, ImportsResolution, resolve_imports};
use crate::skills::discover::{DiscoveredSkill, discover_skills};
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
    #[error("import decl #{index} for `{decl_source}` has no `skills` or `skill` field")]
    InvalidDecl { decl_source: String, index: usize },
    #[error("skipped {count} inadmissible imported skill(s)")]
    RejectedImports { count: usize },
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

        // Spec: docs/spec/skills/selection.md § Canonical shape —
        // "A declaration with neither `skills` nor `skill` is an error."
        for (i, decl) in school.imports.iter().enumerate() {
            if !decl.has_patterns() {
                return Err(PullImportsError::InvalidDecl {
                    decl_source: decl.source.clone(),
                    index: i,
                });
            }
        }

        let skills_dir = self.school_root.join("skills");

        // Discover each unique source once.
        let unique_sources: Vec<&str> = {
            let mut seen: Vec<&str> = Vec::new();
            for d in &school.imports {
                let s = d.source.as_str();
                if !seen.contains(&s) {
                    seen.push(s);
                }
            }
            seen
        };
        let mut discovery: HashMap<String, Vec<DiscoveredSkill>> = HashMap::new();
        for source in &unique_sources {
            ace.progress(&format!("Fetching {source}"));
            let cached = match crate::git::ensure_source_cache(source) {
                Ok(p) => p,
                Err(e) => {
                    ace.warn(&e.to_string());
                    ace.hint(crate::git::auth_hint());
                    return Err(e.into());
                }
            };
            discovery.insert(source.to_string(), discover_skills(&cached)?);
        }

        // Hand off to the imports resolver. It picks per-decl matches,
        // merges across decls (first-wins + warn), and emits provenance.
        let discovery_refs: DiscoveryBySource = discovery
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_slice()))
            .collect();
        let resolution = resolve_imports(&school.imports, &discovery_refs);

        surface_import_diagnostics(ace, &resolution, &school.imports);

        // Build the to-copy set from Included verdicts; map each back to
        // its source's discovered record so we keep the full SKILL.md
        // payload, tier, etc.
        let mut accumulator: Skills<Discovered> = Skills::default();
        let mut rejected_count = 0;
        for resolved in resolution.included() {
            let Some(disc) = discovery.get(&resolved.source) else {
                continue;
            };
            if let Some(d) = disc.iter().find(|d| d.id.as_str() == resolved.identity) {
                if let Err(reason) = d.admission() {
                    rejected_count += 1;
                    ace.warn(&format!(
                        "skipping inadmissible skill `{}` from {}: {reason}",
                        crate::skills::name::render(d.id.as_str()),
                        resolved.source,
                    ));
                    continue;
                }
                let batch = Skills::<Discovered>::from_discovered_with_source(
                    std::slice::from_ref(d),
                    &resolved.source,
                );
                accumulator.merge(batch);
            }
        }

        let winning_names: Vec<String> = accumulator.names().map(String::from).collect();
        let name_refs: Vec<&str> = winning_names.iter().map(String::as_str).collect();
        let changes = accumulator.copy_into(&skills_dir, &name_refs)?;

        let count = changes.len();
        ace.done(&crate::skills::format_pull_summary(&changes));
        if rejected_count > 0 {
            return Err(PullImportsError::RejectedImports {
                count: rejected_count,
            });
        }
        Ok(PullImportsResult::Updated { count })
    }
}

/// Emit per-resolver warnings into the user-visible surface. Collision
/// messages attribute the problem to the school per
/// `docs/spec/skills/selection.md` § Warning boundaries.
fn surface_import_diagnostics(
    ace: &mut Ace,
    resolution: &ImportsResolution,
    decls: &[config::school_toml::ImportDecl],
) {
    for unknown in &resolution.unknown_patterns {
        ace.warn(&format!(
            "no skills matching `{}` in {}",
            unknown.pattern, unknown.source,
        ));
    }
    for collision in &resolution.collisions {
        if collision.suppressed_by_exclude {
            continue;
        }
        ace.warn(&format!(
            "the school you're consuming has a cross-source collision at `{}`: \
             `{}` (decl #{}) wins over `{}` (decl #{}).",
            collision.identity,
            collision.winner_source,
            collision.winner_decl_index,
            collision.loser_source,
            collision.loser_decl_index,
        ));
        ace.hint(
            "add the colliding identity to the winning import's `exclude_skills` \
             to express disjoint sets and silence this warning",
        );
        if collision.frontmatter_mismatch {
            ace.warn(&format!(
                "  + frontmatter `name:` diverges across sources at `{}` — likely upstream spec violation",
                collision.identity,
            ));
        }
    }
    for resolved in &resolution.skills {
        if matches!(resolved.verdict, ImportVerdict::FilteredInternal) {
            // Only the originating decl needs to know; surface once per skill.
            let decl_label = decls
                .get(resolved.decl_index)
                .map(|d| d.source.as_str())
                .unwrap_or("?");
            ace.warn(&format!(
                "skill `{}` in {decl_label} is marked `internal: true`; \
                 set `include_internal = true` on the decl, or import it by explicit name",
                resolved.identity,
            ));
        }
    }
}
