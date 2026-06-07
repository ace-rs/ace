//! Skill-name admission and display rendering.
//!
//! Discovery owns the character gate: every identity segment and the optional
//! frontmatter `name` must pass the Unicode whitelist and structural path-component
//! checks before a skill can be included. Display rendering is the only transform: raw
//! untrusted text remains in the model, and terminal-facing strings go through
//! [`SanitizedString`].

use std::fmt;

mod ucd_tables;

const REPLACEMENT: char = '\u{FFFD}';
const MAX_COMPONENT_BYTES: usize = 255;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SanitizedString(String);

impl SanitizedString {
    #[cfg(test)]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SanitizedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for SanitizedString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RejectReason {
    DisallowedChar {
        context: NameContext,
        value: String,
        codepoint: u32,
        position: usize,
    },
    Empty {
        context: NameContext,
    },
    TooLong {
        context: NameContext,
        value: String,
        bytes: usize,
    },
    DotSegment {
        context: NameContext,
        value: String,
    },
    LeadingDot {
        context: NameContext,
        value: String,
    },
    Slash {
        context: NameContext,
        value: String,
    },
    Backslash {
        context: NameContext,
        value: String,
    },
    Nul {
        context: NameContext,
        value: String,
        position: usize,
    },
}

impl fmt::Display for RejectReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RejectReason::DisallowedChar {
                context,
                value,
                codepoint,
                position,
            } => write!(
                f,
                "{} `{}` contains disallowed U+{codepoint:04X} at character {position}",
                context.label(),
                render(value),
            ),
            RejectReason::Empty { context } => {
                write!(f, "{} is empty", context.label())
            }
            RejectReason::TooLong {
                context,
                value,
                bytes,
            } => write!(
                f,
                "{} `{}` exceeds {MAX_COMPONENT_BYTES} bytes ({bytes})",
                context.label(),
                render(value),
            ),
            RejectReason::DotSegment { context, value } => {
                write!(
                    f,
                    "{} `{}` is a dot segment",
                    context.label(),
                    render(value)
                )
            }
            RejectReason::LeadingDot { context, value } => write!(
                f,
                "{} `{}` starts with a dot",
                context.label(),
                render(value),
            ),
            RejectReason::Slash { context, value } => {
                write!(f, "{} `{}` contains `/`", context.label(), render(value))
            }
            RejectReason::Backslash { context, value } => {
                write!(f, "{} `{}` contains `\\`", context.label(), render(value))
            }
            RejectReason::Nul {
                context,
                value,
                position,
            } => write!(
                f,
                "{} `{}` contains NUL at character {position}",
                context.label(),
                render(value),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NameContext {
    IdentitySegment,
    /// Frontmatter `name:`. Not an admission axis (ACE never emits or matches
    /// on it) — used only to phrase the display-hygiene *warning* raised when
    /// an admitted skill carries a spoofable frontmatter name.
    FrontmatterName,
}

impl NameContext {
    pub fn label(self) -> &'static str {
        match self {
            NameContext::IdentitySegment => "identity segment",
            NameContext::FrontmatterName => "frontmatter name",
        }
    }
}

pub fn render(value: &str) -> SanitizedString {
    let rendered = value
        .chars()
        .map(|c| if char_allowed(c) { c } else { REPLACEMENT })
        .collect();
    SanitizedString(rendered)
}

pub fn char_allowed(c: char) -> bool {
    let cp = c as u32;
    ucd_tables::ALLOWED_NAME_CHARS
        .binary_search_by(|&(start, end)| {
            if cp < start {
                std::cmp::Ordering::Greater
            } else if cp > end {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .is_ok()
}

pub fn structural_ok(name: &str, context: NameContext) -> Result<(), RejectReason> {
    if name.is_empty() {
        return Err(RejectReason::Empty { context });
    }
    if name.len() > MAX_COMPONENT_BYTES {
        return Err(RejectReason::TooLong {
            context,
            value: name.to_string(),
            bytes: name.len(),
        });
    }
    if name == "." || name == ".." {
        return Err(RejectReason::DotSegment {
            context,
            value: name.to_string(),
        });
    }
    if name.starts_with('.') {
        return Err(RejectReason::LeadingDot {
            context,
            value: name.to_string(),
        });
    }
    if name.contains('/') {
        return Err(RejectReason::Slash {
            context,
            value: name.to_string(),
        });
    }
    if name.contains('\\') {
        return Err(RejectReason::Backslash {
            context,
            value: name.to_string(),
        });
    }
    if let Some(position) = name.chars().position(|c| c == '\0') {
        return Err(RejectReason::Nul {
            context,
            value: name.to_string(),
            position,
        });
    }
    Ok(())
}

pub fn admissible_component(name: &str, context: NameContext) -> Result<(), RejectReason> {
    structural_ok(name, context)?;

    for (position, c) in name.chars().enumerate() {
        if !char_allowed(c) {
            return Err(RejectReason::DisallowedChar {
                context,
                value: name.to_string(),
                codepoint: c as u32,
                position,
            });
        }
    }
    Ok(())
}

/// Admissibility of a skill's identity. Identity is the path ACE owns and
/// emits from (name = `basename(identity)`); every segment must clear the
/// whitelist. Frontmatter `name` is deliberately *not* checked: ACE passes it
/// through verbatim and never emits or matches on it, so it is the backend's
/// domain — hostile chars there are neutralized only when ACE renders them
/// (see [`render`]). Boundary: `docs/decisions/2026-06-01-skill-name-is-path.md`.
pub fn admissible_skill(identity: &str) -> Result<(), RejectReason> {
    for segment in identity.split('/') {
        admissible_component(segment, NameContext::IdentitySegment)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_accepts_letters_marks_numbers_punctuation_symbols_and_space() {
        assert!(char_allowed('a'));
        assert!(char_allowed('é'));
        assert!(char_allowed('\u{0301}'));
        assert!(char_allowed('9'));
        assert!(char_allowed('-'));
        assert!(char_allowed('Ω'));
        assert!(char_allowed(' '));
        assert!(char_allowed('🦀'));
    }

    #[test]
    fn whitelist_rejects_control_format_private_unassigned_and_line_separators() {
        assert!(!char_allowed('\u{0007}'));
        assert!(!char_allowed('\u{202E}'));
        assert!(!char_allowed('\u{200D}'));
        assert!(!char_allowed('\u{E000}'));
        assert!(!char_allowed('\u{0378}'));
        assert!(!char_allowed('\u{2028}'));
        assert!(!char_allowed('\u{2029}'));
    }

    #[test]
    fn render_replaces_each_disallowed_char() {
        let sanitized = render("a\u{202E}b\u{0007}c");
        assert_eq!(sanitized.as_str(), "a\u{FFFD}b\u{FFFD}c");
    }

    #[test]
    fn structural_validation_rejects_path_tricks() {
        assert!(matches!(
            structural_ok("..", NameContext::IdentitySegment),
            Err(RejectReason::DotSegment { .. }),
        ));
        assert!(matches!(
            structural_ok(".env", NameContext::IdentitySegment),
            Err(RejectReason::LeadingDot { .. }),
        ));
        assert!(matches!(
            structural_ok("foo/bar", NameContext::IdentitySegment),
            Err(RejectReason::Slash { .. }),
        ));
        assert!(matches!(
            structural_ok("foo\\bar", NameContext::IdentitySegment),
            Err(RejectReason::Backslash { .. }),
        ));
        assert!(matches!(
            structural_ok("foo\0bar", NameContext::IdentitySegment),
            Err(RejectReason::Nul { position: 3, .. }),
        ));
    }

    #[test]
    fn admission_checks_every_identity_segment() {
        assert!(admissible_skill("typescript/coding").is_ok());

        let identity_err = admissible_skill("type\u{202E}script/coding")
            .expect_err("identity segment should reject bidi override");
        assert!(matches!(
            identity_err,
            RejectReason::DisallowedChar {
                context: NameContext::IdentitySegment,
                codepoint: 0x202E,
                position: 4,
                ..
            }
        ));
    }

    #[test]
    fn admission_ignores_frontmatter_name() {
        // Frontmatter is the backend's domain; a hostile `name:` does not
        // reject a skill whose identity path is clean.
        assert!(admissible_skill("typescript/coding").is_ok());
    }

    #[test]
    fn reject_reason_display_sanitizes_untrusted_name() {
        let err = admissible_skill("bad\u{202E}name").expect_err("reject");
        let rendered = err.to_string();
        assert!(rendered.contains("bad\u{FFFD}name"));
        assert!(rendered.contains("U+202E"));
    }
}
