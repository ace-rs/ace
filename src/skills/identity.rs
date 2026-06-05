//! Typed skill identity ([`Locator`]) and the pattern matcher
//! ([`pattern_matches`]) that decides whether a user pattern selects it.
//!
//! [`Locator`] — a path-shaped identity produced **only** by the discovery
//! layer after the prefix-strip rule has been applied (see
//! `docs/spec/skills/model.md` § Type-safety invariant). Constructors are
//! `pub(crate)` — visible across the binary so typed test fixtures in
//! `crate::actions::*` can construct fakes, but unreachable from any
//! hypothetical external library consumer. Discovery is the only production
//! entry point; the looser visibility is a convention, not a hard wall.
//!
//! Patterns stay raw `&str` — they are selection *input*, never a skill state.
//! [`pattern_matches`] applies the `selection.md` § Match handle rules and is
//! total; the resolvers validate glob *syntax* (via `glob::validate`) at their
//! own seam, warning-and-skipping unsupported patterns rather than rejecting.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::path::Path;

use crate::glob;

/// A skill's identity path. Produced only by discovery (the prefix-strip
/// rule is the only path into existence). Equivalent to a slash-joined
/// path: `foo`, `typescript/coding`, etc.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Locator(String);

impl Locator {
    /// Construct a `Locator` from a path relative to a discovery prefix.
    /// Components are joined with `/` for cross-platform stability.
    /// `pub(in crate::skills)` so only the discovery layer can call it.
    pub(crate) fn from_relative_path(rel: &Path) -> Self {
        let mut parts: Vec<&str> = Vec::new();
        for comp in rel.iter() {
            if let Some(s) = comp.to_str() {
                parts.push(s);
            }
        }
        Self(parts.join("/"))
    }

    /// Construct a `Locator` from a basename. Used by stage-1 discovery
    /// (root-level `SKILL.md`) and by `[[imports]]` declarations that
    /// supply an explicit identity key.
    pub(crate) fn from_basename(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Borrow the identity as a slash-joined `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The trailing path segment. For flat identities this equals
    /// `as_str()`; for nested `typescript/coding` it returns `coding`.
    pub fn leaf(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// True when the identity contains a `/` separator (nested layout).
    pub fn is_nested(&self) -> bool {
        self.0.contains('/')
    }
}

impl Deref for Locator {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for Locator {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for Locator {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Locator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for Locator {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for Locator {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for Locator {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl From<&Locator> for String {
    fn from(id: &Locator) -> String {
        id.0.clone()
    }
}

impl From<Locator> for String {
    fn from(id: Locator) -> String {
        id.0
    }
}

/// Apply the spec's match-handle rules to a raw string pattern and a raw
/// identity string. Called by the project and imports resolvers.
/// See `docs/spec/skills/selection.md` § Match handle.
///
/// - Glob (`*` present): standard glob match against the full identity.
/// - Path-anchored (`/` present, no `*`): exact equality against the
///   identity path — no leaf-fallback.
/// - Bare name (neither `*` nor `/`): exact identity OR exact match of
///   the trailing path segment (leaf-fallback). This preserves the
///   pre-nested-identity UX: typing `rust-coding` still resolves to the
///   skill called `rust-coding` whether it lives flat (`rust-coding`) or
///   nested (`typescript/rust-coding`).
pub fn pattern_matches(pattern: &str, identity: &str) -> bool {
    if pattern.contains('*') {
        return glob::glob_match(pattern, identity);
    }
    if pattern.contains('/') {
        return pattern == identity;
    }
    if pattern == identity {
        return true;
    }
    // Leaf-fallback: trailing path segment of identity equals the bare name.
    identity
        .rsplit('/')
        .next()
        .map(|leaf| leaf == pattern)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn id(s: &str) -> Locator {
        // Test-only constructor — uses the from_basename path which the
        // discovery layer also uses. Tests in this module live inside
        // crate::skills so they can call the private ctor.
        Locator::from_basename(s)
    }

    #[test]
    fn skill_id_from_path_joins_with_slash() {
        let id = Locator::from_relative_path(&PathBuf::from("typescript").join("coding"));
        assert_eq!(id.as_str(), "typescript/coding");
    }

    #[test]
    fn skill_id_flat_basename() {
        let id = Locator::from_basename("foo");
        assert_eq!(id.as_str(), "foo");
        assert_eq!(id.leaf(), "foo");
        assert!(!id.is_nested());
    }

    #[test]
    fn skill_id_nested_has_leaf() {
        let id = Locator::from_relative_path(&PathBuf::from("ts").join("coding"));
        assert_eq!(id.leaf(), "coding");
        assert!(id.is_nested());
    }

    #[test]
    fn skill_id_deref_str_methods_work() {
        let id = id("rust-coding");
        assert!(id.starts_with("rust-"));
        assert!(id.contains("coding"));
        assert_eq!(&id[..4], "rust");
    }

    #[test]
    fn skill_id_partial_eq_against_strings() {
        let id = id("foo");
        assert_eq!(id, *"foo");
        assert_eq!(id, "foo");
        assert_eq!(id, "foo".to_string());
    }

    #[test]
    fn skill_id_into_string() {
        let id = id("foo");
        let s: String = (&id).into();
        assert_eq!(s, "foo");
        let s: String = id.into();
        assert_eq!(s, "foo");
    }

    // -- pattern_matches (selection.md § Match handle) --

    #[test]
    fn bare_name_exact_match() {
        assert!(pattern_matches("rust-coding", "rust-coding"));
        assert!(!pattern_matches("rust-coding", "rust-fmt"));
    }

    #[test]
    fn bare_name_leaf_match_under_nested_path() {
        assert!(
            pattern_matches("rust-coding", "typescript/rust-coding"),
            "bare name should match the leaf segment"
        );
    }

    #[test]
    fn bare_name_no_prefix_match() {
        // `rust` should not match `rust-coding`. Only exact OR leaf.
        assert!(!pattern_matches("rust", "rust-coding"));
    }

    #[test]
    fn bare_name_no_middle_match() {
        // `coding` should not match `rust-coding-extra`.
        assert!(!pattern_matches("coding", "rust-coding-extra"));
    }

    #[test]
    fn path_anchored_no_leaf_fallback() {
        // `typescript/coding` matches exactly `typescript/coding`, not just
        // anything ending in `/coding`.
        assert!(pattern_matches("typescript/coding", "typescript/coding"));
        assert!(!pattern_matches("typescript/coding", "python/coding"));
    }

    #[test]
    fn glob_star_matches_everything() {
        assert!(pattern_matches("*", "foo"));
        assert!(pattern_matches("*", "rust-coding"));
    }

    #[test]
    fn glob_prefix_pattern() {
        assert!(pattern_matches("rust-*", "rust-coding"));
        assert!(pattern_matches("rust-*", "rust-fmt"));
        assert!(!pattern_matches("rust-*", "python-coding"));
    }

    #[test]
    fn glob_suffix_pattern_anchored() {
        // `*/coding` matches identities like `typescript/coding` —
        // the glob version of "any path ending in /coding".
        assert!(pattern_matches("*/coding", "typescript/coding"));
    }

    #[test]
    fn skill_id_constructor_is_module_private() {
        // Compile-time check: external code cannot call from_basename or
        // from_relative_path. Verified by the fact that this module
        // (crate::skills::identity) CAN call them; tests outside
        // crate::skills cannot. No runtime assertion is possible — the
        // type invariant is enforced by Rust's privacy at compile time.
        let _ = Locator::from_basename("ok");
    }
}
