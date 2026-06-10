use std::path::Path;

use crate::ace::Ace;
use crate::actions::school::gitlink::{gitlinked_names, warn_broken_submodule};
use crate::config;
use crate::skills::discover::discover_skills;
use crate::skills::resolve::{Discovery, ImportVerdict, ImportsResolution, resolve_imports};
use crate::skills::{Discovered, FRONTMATTER_WARNING_HINT, Skill, Skills, name};

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
    #[error("skipped {count} skill(s) committed as broken git submodules")]
    BrokenSubmodules { count: usize },
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
            let (skills, prunes) = discover_skills(&cached)?;
            for reason in &prunes {
                ace.warn(&format!(
                    "skipping malformed skill identity from {source}: {reason}"
                ));
            }
            discovery.insert(source, skills);
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

        // A previous buggy import may have committed a skill dir as a gitlink
        // (an accidental submodule from a leaked `.git`). ACE will not rewrite
        // the user's index — it warns, skips the poisoned skill so it never
        // writes files into a submodule-tracked path, and points at the fix.
        // Everything healthy still syncs.
        let winning_names: Vec<String> = validated.names().map(String::from).collect();
        let gitlinked = gitlinked_names(self.school_root, &winning_names);
        for name in &gitlinked {
            warn_broken_submodule(ace, name);
        }

        let healthy: Vec<&str> = winning_names
            .iter()
            .map(String::as_str)
            .filter(|n| !gitlinked.iter().any(|g| g == n))
            .collect();
        let changes = validated.copy_into(&skills_dir, &healthy)?;

        ace.done(&crate::skills::format_pull_summary(&changes));
        if !gitlinked.is_empty() {
            return Err(PullImportsError::BrokenSubmodules {
                count: gitlinked.len(),
            });
        }
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
    // Patterns and source labels are raw third-party `String`s (not `Locator`s,
    // whose Display self-sanitizes), so they go through `name::render` here.
    for unknown in &resolution.unknown_patterns {
        ace.warn(&format!(
            "no skills matching `{}` in {}",
            name::render(&unknown.pattern),
            name::render(&unknown.source),
        ));
    }
    for invalid in &resolution.invalid_patterns {
        ace.warn(&format!(
            "ignoring unsupported import pattern `{}` in {}: {}",
            name::render(&invalid.pattern),
            name::render(&invalid.source),
            invalid.reason,
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
            name::render(&collision.winner_source),
            collision.winner_decl_index,
            name::render(&collision.loser_source),
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
                "skill `{}` in {} is marked `internal: true`; \
                 set `include_internal = true` on the decl, or import it by explicit name",
                resolved.identity,
                name::render(decl_label),
            ));
        }
    }
}
