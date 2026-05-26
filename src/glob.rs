//! Simple glob matching for skill name patterns.
//!
//! Supports `*` as a wildcard matching zero or more characters.
//! No `?`, `**`, or character classes.
//!
//! Examples:
//! - `"*"` matches everything
//! - `"frontend-*"` matches `"frontend-design"`, `"frontend-review"`
//! - `"*-coding"` matches `"go-coding"`, `"rust-coding"`
//! - `"*-design-*"` matches `"frontend-design-system"`
//! - `"go-coding"` matches only `"go-coding"` (exact)

/// Returns true if `pattern` contains a `*` wildcard.
pub fn is_glob(pattern: &str) -> bool {
    pattern.contains('*')
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GlobError {
    #[error("`?` is not a supported wildcard")]
    Question,
    #[error("character classes (`[...]`) are not supported")]
    CharClass,
    #[error("pattern is empty")]
    Empty,
}

/// Reject patterns whose syntax goes beyond what `glob_match` supports.
///
/// Accepts literals and `*`. `**` is accepted and treated as `*` (per
/// `docs/spec/skills/selection.md` § Glob patterns table — "**: same as
/// *, accepted, not special"). `?` and `[...]` are rejected so CLI users
/// see a clear error at entry instead of a silent zero-match later.
pub fn validate(pattern: &str) -> Result<(), GlobError> {
    if pattern.is_empty() {
        return Err(GlobError::Empty);
    }
    if pattern.contains('?') {
        return Err(GlobError::Question);
    }
    if pattern.contains('[') || pattern.contains(']') {
        return Err(GlobError::CharClass);
    }
    Ok(())
}

/// Match `name` against a glob `pattern`.
///
/// Splits the pattern on `*` and checks that all literal parts appear
/// in order within the name. Leading/trailing `*` allow prefix/suffix
/// flexibility.
pub fn glob_match(pattern: &str, name: &str) -> bool {
    if !is_glob(pattern) {
        return pattern == name;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    let mut pos = 0;

    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        // First part must be a prefix if pattern doesn't start with *
        if i == 0 {
            let Some(rest) = name.strip_prefix(part) else {
                return false;
            };
            pos = name.len() - rest.len();
            continue;
        }

        // Last part must be a suffix if pattern doesn't end with *
        if i == parts.len() - 1 {
            return name[pos..].ends_with(part);
        }

        // Middle parts: find next occurrence after current position
        let Some(found) = name[pos..].find(part) else {
            return false;
        };
        pos += found + part.len();
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_matches_everything() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn exact_match_without_glob() {
        assert!(glob_match("go-coding", "go-coding"));
        assert!(!glob_match("go-coding", "rust-coding"));
    }

    #[test]
    fn prefix_glob() {
        assert!(glob_match("frontend-*", "frontend-design"));
        assert!(glob_match("frontend-*", "frontend-"));
        assert!(!glob_match("frontend-*", "backend-design"));
    }

    #[test]
    fn suffix_glob() {
        assert!(glob_match("*-coding", "go-coding"));
        assert!(glob_match("*-coding", "rust-coding"));
        assert!(!glob_match("*-coding", "go-review"));
    }

    #[test]
    fn middle_glob() {
        assert!(glob_match("front*end", "frontend"));
        assert!(glob_match("front*end", "front-and-backend"));
        assert!(!glob_match("front*end", "frontend-design"));
    }

    #[test]
    fn multiple_globs() {
        assert!(glob_match("*-design-*", "frontend-design-system"));
        assert!(glob_match("*-design-*", "x-design-y"));
        assert!(!glob_match("*-design-*", "frontend-review-system"));
    }

    #[test]
    fn is_glob_detects_star() {
        assert!(is_glob("*"));
        assert!(is_glob("frontend-*"));
        assert!(!is_glob("go-coding"));
    }

    #[test]
    fn consecutive_stars() {
        assert!(glob_match("**", "anything"));
        assert!(glob_match("a**b", "axyzb"));
    }

    #[test]
    fn no_match_empty_name_with_literal() {
        assert!(!glob_match("abc", ""));
        assert!(!glob_match("*-coding", ""));
    }

    #[test]
    fn validate_accepts_literals_and_star() {
        assert_eq!(validate("rust-coding"), Ok(()));
        assert_eq!(validate("*"), Ok(()));
        assert_eq!(validate("frontend-*"), Ok(()));
        assert_eq!(validate("*-coding"), Ok(()));
        assert_eq!(validate("*-design-*"), Ok(()));
    }

    #[test]
    fn validate_accepts_double_star() {
        // Per docs/spec/skills/selection.md: `**` is accepted, treated
        // as `*`. The matcher already handles consecutive stars; the
        // validator no longer rejects.
        assert_eq!(validate("**"), Ok(()));
        assert_eq!(validate("a**b"), Ok(()));
        assert_eq!(validate("**/foo"), Ok(()));
    }

    #[test]
    fn validate_rejects_question_mark() {
        assert_eq!(validate("foo?"), Err(GlobError::Question));
        assert_eq!(validate("?bar"), Err(GlobError::Question));
    }

    #[test]
    fn validate_rejects_char_classes() {
        assert_eq!(validate("[abc]"), Err(GlobError::CharClass));
        assert_eq!(validate("foo[0-9]"), Err(GlobError::CharClass));
        assert_eq!(validate("foo]"), Err(GlobError::CharClass));
    }

    #[test]
    fn validate_rejects_empty() {
        assert_eq!(validate(""), Err(GlobError::Empty));
    }
}
