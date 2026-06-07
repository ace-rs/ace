use std::path::Path;

use crate::ace::Ace;
use crate::config;
use crate::skills::discover::discover_skills;
use crate::skills::resolve::{resolve_imports, Discovery, ImportVerdict, ImportsResolution};
use crate::skills::{name, Discovered, Skill, Skills, FRONTMATTER_WARNING_HINT};

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
    Updated,
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

        // Discover each source once, indexed for O(1) lookup. A source can
        // appear in multiple decls; fetch it only the first time we see it.
        let mut discovery = Discovery::default();
        for decl in &school.imports {
            let source = decl.source.as_str();
            if discovery.has_source(source) {
                continue;
            }
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

        // Hand off to the imports resolver: per-decl matches, cross-decl merge
        // (first-wins + warn), provenance.
        let resolution = resolve_imports(&school.imports, &discovery);
        surface_import_diagnostics(ace, &resolution, &school.imports);

        // Map each Included verdict back to its source's discovered record
        // (full SKILL.md payload, tier, frontmatter) via the indexed lookup,
        // tagging it with the source it won under. `included()` is already
        // unique by identity, so there is nothing to dedup.
        let mut found: Vec<Skill<Discovered>> = Vec::new();
        for resolved in resolution.included() {
            let Some(d) = discovery.lookup(&resolved.source, &resolved.identity) else {
                continue;
            };
            if let Some(warning) = d.frontmatter_warning() {
                ace.warn(&warning);
                ace.hint(FRONTMATTER_WARNING_HINT);
            }
            found.push(Skill {
                source: Some(resolved.source.clone()),
                ..d.clone()
            });
        }

        // The admission gate: inadmissible identities partition off here, so
        // only validated skills reach `copy_into`.
        let (validated, rejected) = Skills::from_skills(found).validate();
        for r in &rejected {
            let from = r.source.as_deref().unwrap_or("?");
            ace.warn(&format!(
                "skipping inadmissible skill `{}` from {from}: {}",
                name::render(r.locator.as_str()),
                r.reason,
            ));
        }
        let rejected_count = rejected.len();

        let winning_names: Vec<String> = validated.names().map(String::from).collect();
        let name_refs: Vec<&str> = winning_names.iter().map(String::as_str).collect();
        let changes = validated.copy_into(&skills_dir, &name_refs)?;

        ace.done(&crate::skills::format_pull_summary(&changes));
        if rejected_count > 0 {
            return Err(PullImportsError::RejectedImports {
                count: rejected_count,
            });
        }
        Ok(PullImportsResult::Updated)
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
    for invalid in &resolution.invalid_patterns {
        ace.warn(&format!(
            "ignoring unsupported import pattern `{}` in {}: {}",
            invalid.pattern, invalid.source, invalid.reason,
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
