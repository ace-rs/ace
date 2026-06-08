//! Typed skill identity ([`Locator`]) and the pattern matcher
//! ([`pattern_matches`]) that decides whether a user pattern selects it.
//!
//! [`Locator`] — a path-shaped identity produced after the prefix-strip rule
//! (see `docs/spec/skills/model.md` § Type-safety invariant). The production
//! doors are the *fallible* [`Locator::try_from_path`] / [`try_from_basename`]:
//! they structurally validate every segment, so a constructed `Locator` is a
//! sound path-component sequence by construction. Character admissibility is a
//! separate axis, settled later at `validate` (proven by `Skill<Validated>`).
//! Discovery is the only production caller; the infallible `from_*` fixtures are
//! `#[cfg(test)]`. The newtype itself is the real guard against pattern→identity
//! collapse — there is no implicit `String`→`Locator` conversion.
//!
//! Patterns stay raw `&str` — they are selection *input*, never a skill state.
//! [`pattern_matches`] applies the `selection.md` § Match handle rules and is
//! total; the resolvers validate glob *syntax* (via `glob::validate`) at their
//! own seam, warning-and-skipping unsupported patterns rather than rejecting.

use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::path::Path;

use super::name::{NameContext, RejectReason, structural_ok};
use crate::glob;

/// A skill's identity path. Produced only by discovery (the prefix-strip
/// rule is the only path into existence). Equivalent to a slash-joined
/// path: `foo`, `typescript/coding`, etc.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Locator(String);

impl Locator {
    /// Construct a `Locator` from a path relative to a discovery prefix,
    /// validating each component's structure. Components are joined with `/`
    /// for cross-platform stability. The sole production door from a discovery
    /// path: a returned `Locator` is structurally sound by construction
    /// (see `docs/spec/skills/model.md` § Type-safety invariant). Structural
    /// failure (e.g. a backslash in a segment) is `Err`; the caller prunes.
    /// Character admissibility is a *separate* axis settled later at `validate`.
    pub(crate) fn try_from_path(rel: &Path) -> Result<Self, RejectReason> {
        let mut parts: Vec<&str> = Vec::new();
        for comp in rel.iter() {
            if let Some(s) = comp.to_str() {
                structural_ok(s, NameContext::IdentitySegment)?;
                parts.push(s);
            }
        }
        Ok(Self(parts.join("/")))
    }

    /// Construct a `Locator` from a (possibly slash-joined) basename, validating
    /// each segment's structure. Used by stage-1 discovery (root-level
    /// `SKILL.md`). Same structural guarantee as [`try_from_path`](Self::try_from_path).
    pub(crate) fn try_from_basename(name: impl Into<String>) -> Result<Self, RejectReason> {
        let name = name.into();
        for segment in name.split('/') {
            structural_ok(segment, NameContext::IdentitySegment)?;
        }
        Ok(Self(name))
    }

    /// Infallible test fixture — unwraps [`try_from_basename`](Self::try_from_basename).
    /// `#[cfg(test)]` so production identities only ever come from the fallible
    /// doors; test code crate-wide can still mint fakes cheaply.
    #[cfg(test)]
    pub(crate) fn from_basename(name: impl Into<String>) -> Self {
        Self::try_from_basename(name).expect("valid test locator")
    }

    /// Infallible test fixture — unwraps [`try_from_path`](Self::try_from_path).
    #[cfg(test)]
    pub(crate) fn from_relative_path(rel: &Path) -> Self {
        Self::try_from_path(rel).expect("valid test locator")
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
    /// Human-facing axis: sanitized. A `Locator` may be pre-validation
    /// (discovery → diagnostics surface before `validate`), so its identity can
    /// still carry chars the whitelist rejects. Display routes through
    /// [`name::render`] so any `{}`/`to_string()` of a `Locator` is safe to put
    /// on a terminal by construction — no caller has to remember. Raw,
    /// machine-faithful access stays on [`as_str`](Self::as_str). Post-validate
    /// the identity is admissible, so this is a no-op there.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", super::name::render(&self.0))
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
    fn display_sanitizes_hostile_identity_raw_stays_on_as_str() {
        // A pre-validation Locator can carry disallowed chars — char admission
        // is settled later at `validate`, structural construction lets bidi
        // through. Display is the human-facing axis and must neutralize them;
        // `as_str` is the raw machine axis and preserves them verbatim.
        let loc = Locator::from_basename("bad\u{202E}name");
        assert_eq!(loc.to_string(), "bad\u{FFFD}name", "Display sanitizes");
        assert_eq!(loc.as_str(), "bad\u{202E}name", "as_str stays raw");
    }

    #[test]
    fn try_from_basename_validates_structure() {
        // The production door is fallible: a clean basename constructs, a
        // structurally-unsafe one (backslash) is rejected. There is no
        // implicit `String`→`Locator` conversion — construction is explicit
        // and structurally checked, which is the real invariant (not privacy).
        assert!(Locator::try_from_basename("ok").is_ok());
        assert!(matches!(
            Locator::try_from_basename("bad\\seg"),
            Err(RejectReason::Backslash { .. })
        ));
    }

    #[test]
    fn try_from_path_rejects_structurally_unsafe_segment() {
        assert!(Locator::try_from_path(&PathBuf::from("ts").join("coding")).is_ok());
        assert!(Locator::try_from_path(&PathBuf::from("a\\b")).is_err());
    }
}
