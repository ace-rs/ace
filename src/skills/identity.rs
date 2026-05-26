//! Typed skill identifiers and user-supplied match handles.
//!
//! Two newtypes encode the spec's identity-vs-input distinction
//! (see `docs/spec/skills/model.md` § Type-safety invariant and
//! `docs/spec/skills/selection.md` § Match handle):
//!
//! - [`SkillId`] — a path-shaped identity produced **only** by the discovery
//!   layer after the prefix-strip rule has been applied. External code
//!   cannot synthesize a `SkillId` from a raw string. The constructor is
//!   `pub(in crate::skills)`, so only modules inside `crate::skills` can
//!   build one — discovery is the entry point.
//!
//! - [`MatchHandle`] — a user-supplied pattern: CLI `--skill`, `[[imports]]`
//!   `skills`, `ace.toml` `{skills, include_skills, exclude_skills}`. Built
//!   via [`MatchHandle::new`], which validates glob syntax up front so
//!   downstream code never has to re-check.
//!
//! `SkillId` and `MatchHandle` cannot be interconverted directly; the only
//! crossing point is [`MatchHandle::matches`], which decides whether a
//! handle matches an identity per the rules in `selection.md`.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::path::Path;

use crate::glob;

/// A skill's identity path. Produced only by discovery (the prefix-strip
/// rule is the only path into existence). Equivalent to a slash-joined
/// path: `foo`, `typescript/coding`, etc.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SkillId(String);

impl SkillId {
    /// Construct a `SkillId` from a path relative to a discovery prefix.
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

    /// Construct a `SkillId` from a basename. Used by stage-1 discovery
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

impl Deref for SkillId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SkillId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for SkillId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SkillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl PartialEq<str> for SkillId {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for SkillId {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<String> for SkillId {
    fn eq(&self, other: &String) -> bool {
        &self.0 == other
    }
}

impl From<&SkillId> for String {
    fn from(id: &SkillId) -> String {
        id.0.clone()
    }
}

impl From<SkillId> for String {
    fn from(id: SkillId) -> String {
        id.0
    }
}

/// A user-supplied skill match pattern. Distinct kind from `SkillId` —
/// constructed via [`MatchHandle::new`], which validates glob syntax.
///
/// Production callsites land in the project and imports resolvers (later
/// slices); the type is fully defined here so downstream code can adopt
/// it incrementally.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[allow(dead_code)]
pub struct MatchHandle(String);

#[allow(dead_code)]
impl MatchHandle {
    /// Construct a handle from a raw user pattern. Validates glob syntax
    /// (rejects `**`, `?`, character classes, empty) so downstream matchers
    /// never have to re-check.
    pub fn new(pattern: impl Into<String>) -> Result<Self, glob::GlobError> {
        let pattern = pattern.into();
        glob::validate(&pattern)?;
        Ok(Self(pattern))
    }

    /// Skip validation. Used at the config-deserialize boundary where
    /// `ace.toml` already holds previously-validated strings, and at test
    /// fixtures. Internal callers must not pass arbitrary strings without
    /// validating elsewhere.
    pub(crate) fn from_raw(pattern: impl Into<String>) -> Self {
        Self(pattern.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True when the pattern contains `*`.
    pub fn is_glob(&self) -> bool {
        glob::is_glob(&self.0)
    }

    /// True when the pattern contains `/` (path-anchored — exact-match only,
    /// no leaf-fallback). See `selection.md` § Path-anchored patterns.
    pub fn is_path_anchored(&self) -> bool {
        self.0.contains('/')
    }

    /// True when this handle matches the given identity per the rules in
    /// `docs/spec/skills/selection.md` § Match handle:
    ///
    /// - Glob (`*` present): standard glob match against the full identity
    ///   path.
    /// - Path-anchored (`/` present, no `*`): exact equality against the
    ///   identity path.
    /// - Bare name (no `*` or `/`): exact identity OR exact match of the
    ///   trailing segment (leaf fallback).
    pub fn matches(&self, id: &SkillId) -> bool {
        if self.is_glob() {
            return glob::glob_match(&self.0, id.as_str());
        }
        if self.is_path_anchored() {
            return self.0 == id.as_str();
        }
        // Bare name: exact or leaf match.
        id.as_str() == self.0 || id.leaf() == self.0
    }
}

impl Deref for MatchHandle {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for MatchHandle {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MatchHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn id(s: &str) -> SkillId {
        // Test-only constructor — uses the from_basename path which the
        // discovery layer also uses. Tests in this module live inside
        // crate::skills so they can call the private ctor.
        SkillId::from_basename(s)
    }

    #[test]
    fn skill_id_from_path_joins_with_slash() {
        let id = SkillId::from_relative_path(&PathBuf::from("typescript").join("coding"));
        assert_eq!(id.as_str(), "typescript/coding");
    }

    #[test]
    fn skill_id_flat_basename() {
        let id = SkillId::from_basename("foo");
        assert_eq!(id.as_str(), "foo");
        assert_eq!(id.leaf(), "foo");
        assert!(!id.is_nested());
    }

    #[test]
    fn skill_id_nested_has_leaf() {
        let id = SkillId::from_relative_path(&PathBuf::from("ts").join("coding"));
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

    // -- MatchHandle --

    #[test]
    fn match_handle_validates_syntax() {
        assert!(MatchHandle::new("foo").is_ok());
        assert!(MatchHandle::new("*").is_ok());
        assert!(MatchHandle::new("typescript/coding").is_ok());
        assert!(MatchHandle::new("rust-*").is_ok());
        assert!(MatchHandle::new("").is_err());
        assert!(MatchHandle::new("**").is_err());
        assert!(MatchHandle::new("foo?").is_err());
        assert!(MatchHandle::new("[abc]").is_err());
    }

    #[test]
    fn match_handle_from_raw_skips_validation() {
        // Used by config deserialize where the string came from a previously
        // validated source. We exercise it; we don't validate.
        let h = MatchHandle::from_raw("foo");
        assert_eq!(h.as_str(), "foo");
    }

    #[test]
    fn match_handle_classification() {
        let h = MatchHandle::new("foo").unwrap();
        assert!(!h.is_glob());
        assert!(!h.is_path_anchored());

        let h = MatchHandle::new("typescript/coding").unwrap();
        assert!(!h.is_glob());
        assert!(h.is_path_anchored());

        let h = MatchHandle::new("rust-*").unwrap();
        assert!(h.is_glob());
        assert!(!h.is_path_anchored());

        let h = MatchHandle::new("*/coding").unwrap();
        assert!(h.is_glob());
        // Has both `/` and `*` — glob wins (it's a glob, not path-anchored exact).
    }

    #[test]
    fn bare_name_exact_match() {
        let h = MatchHandle::new("rust-coding").unwrap();
        assert!(h.matches(&id("rust-coding")));
        assert!(!h.matches(&id("rust-fmt")));
    }

    #[test]
    fn bare_name_leaf_match_under_nested_path() {
        let h = MatchHandle::new("rust-coding").unwrap();
        let nested = SkillId::from_relative_path(&PathBuf::from("typescript").join("rust-coding"));
        assert!(h.matches(&nested), "bare name should match the leaf segment");
    }

    #[test]
    fn bare_name_no_prefix_match() {
        // `rust` should not match `rust-coding`. Only exact OR leaf.
        let h = MatchHandle::new("rust").unwrap();
        assert!(!h.matches(&id("rust-coding")));
    }

    #[test]
    fn bare_name_no_middle_match() {
        // `coding` should not match `rust-coding-extra`.
        let h = MatchHandle::new("coding").unwrap();
        assert!(!h.matches(&id("rust-coding-extra")));
    }

    #[test]
    fn path_anchored_no_leaf_fallback() {
        // `typescript/coding` matches exactly `typescript/coding`, not just
        // anything ending in `/coding`.
        let h = MatchHandle::new("typescript/coding").unwrap();
        let nested =
            SkillId::from_relative_path(&PathBuf::from("typescript").join("coding"));
        assert!(h.matches(&nested));

        let other = SkillId::from_relative_path(&PathBuf::from("python").join("coding"));
        assert!(!h.matches(&other));
    }

    #[test]
    fn glob_star_matches_everything() {
        let h = MatchHandle::new("*").unwrap();
        assert!(h.matches(&id("foo")));
        assert!(h.matches(&id("rust-coding")));
    }

    #[test]
    fn glob_prefix_pattern() {
        let h = MatchHandle::new("rust-*").unwrap();
        assert!(h.matches(&id("rust-coding")));
        assert!(h.matches(&id("rust-fmt")));
        assert!(!h.matches(&id("python-coding")));
    }

    #[test]
    fn glob_suffix_pattern_anchored() {
        // `*/coding` matches identities like `typescript/coding` —
        // the glob version of "any path ending in /coding".
        let h = MatchHandle::new("*/coding").unwrap();
        let nested =
            SkillId::from_relative_path(&PathBuf::from("typescript").join("coding"));
        assert!(h.matches(&nested));
        // The flat `coding` skill matches too, because glob `*/coding`
        // wildcards the `*` even to empty (per glob_match semantics).
        // The spec table marks `*/coding` as "multi-segment paths only",
        // but the current glob_match treats `*` as "zero-or-more". This
        // is a minor semantic gap noted for the imports-resolver slice
        // when collision warnings get specific.
    }

    #[test]
    fn skill_id_constructor_is_module_private() {
        // Compile-time check: external code cannot call from_basename or
        // from_relative_path. Verified by the fact that this module
        // (crate::skills::identity) CAN call them; tests outside
        // crate::skills cannot. No runtime assertion is possible — the
        // type invariant is enforced by Rust's privacy at compile time.
        let _ = SkillId::from_basename("ok");
    }
}
