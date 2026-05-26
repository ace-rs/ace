//! Unicode sanitization at write/display boundaries.
//!
//! Threat model (per `docs/spec/skills/model.md` § Sanitization): malformed
//! or malicious SKILL.md frontmatter may carry terminal escape sequences
//! (CWE-150) or bidi-override chars (U+202A–U+202E, U+2066–U+2069) that
//! spoof display when rendered to a terminal or written into a backend
//! SKILL.md.
//!
//! Boundary policy (spec § Boundary policy):
//!
//! | Boundary                       | Action                       |
//! | ------------------------------ | ---------------------------- |
//! | ACE's own display              | sanitize on render           |
//! | School-storage write           | preserve verbatim            |
//! | Backend emit write (link name) | sanitize before write        |
//! | Internal in-memory model       | raw, preserved               |
//!
//! Spec § Approach calls for a Unicode-class whitelist: allow `L*` (letter),
//! `M*` (mark), `N*` (number), `P*` (punctuation), `S*` (symbol), `Zs`
//! (space). Drop `C*` (control) and the bidi-override block.
//!
//! Implementation note: we avoid a `unicode-general-category` dependency
//! by reducing the rule to its operational equivalent — drop `Cc` (via
//! `char::is_control`) plus the explicit bidi-override block. The other
//! C-class subcategories (Cn unassigned, Co private-use, Cs surrogate)
//! don't appear in well-formed Rust strings and aren't the threat model;
//! Cf format chars outside the bidi range are deliberately preserved
//! (e.g. zero-width joiners in glyph composition). Revisit if a doctor
//! check needs the stricter form.

/// Sanitize a string for rendering into ACE's terminal output. Used at
/// `ace.warn`/`hint`/etc. boundaries when the input may carry untrusted
/// frontmatter content (skill names, descriptions, error messages built
/// from third-party metadata).
pub fn for_terminal(s: &str) -> String {
    s.chars().filter(|c| !is_dangerous(*c)).collect()
}

/// Sanitize a string for use as a file path component or written
/// frontmatter value at the backend-emit boundary. Same rule as
/// [`for_terminal`]; the separation is named so callsite intent is
/// explicit.
pub fn for_emit(s: &str) -> String {
    s.chars().filter(|c| !is_dangerous(*c)).collect()
}

/// True when `c` should be dropped at sanitization boundaries.
fn is_dangerous(c: char) -> bool {
    if c.is_control() {
        return true;
    }
    matches!(
        c,
        '\u{202A}'..='\u{202E}'   // LRE/RLE/PDF/LRO/RLO
        | '\u{2066}'..='\u{2069}' // LRI/RLI/FSI/PDI
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_ascii_passes_through() {
        assert_eq!(for_terminal("rust-coding"), "rust-coding");
        assert_eq!(for_emit("typescript-coding"), "typescript-coding");
    }

    #[test]
    fn unicode_letters_pass_through() {
        assert_eq!(for_terminal("café"), "café");
        assert_eq!(for_terminal("日本語"), "日本語");
        assert_eq!(for_terminal("Ωmega"), "Ωmega");
    }

    #[test]
    fn ascii_control_chars_dropped() {
        // Bell, backspace, escape, etc.
        let raw = "foo\x07bar\x1bbaz\x7fend";
        assert_eq!(for_terminal(raw), "foobarbazend");
    }

    #[test]
    fn ansi_escape_sequence_dropped() {
        let raw = "\x1b[31mred\x1b[0m";
        // The escape byte is gone; surrounding `[31m`, `red`, `[0m` stay
        // (those are legal punctuation + letters). Result is the visible
        // bytes minus the control character itself.
        assert_eq!(for_terminal(raw), "[31mred[0m");
    }

    #[test]
    fn newlines_and_tabs_dropped() {
        // These are control chars too — they don't belong in a single-line
        // skill name regardless of display intent.
        assert_eq!(for_terminal("a\nb"), "ab");
        assert_eq!(for_terminal("a\tb"), "ab");
    }

    #[test]
    fn bidi_overrides_dropped() {
        // U+202E = RIGHT-TO-LEFT OVERRIDE — the classic filename-spoof char.
        let spoof = "innocent\u{202E}gnp.exe";
        assert_eq!(for_terminal(spoof), "innocentgnp.exe");
        // Full bidi-control range.
        for cp in [
            '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}',
            '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
        ] {
            let s = format!("a{cp}b");
            assert_eq!(for_terminal(&s), "ab", "bidi {cp:?} should drop");
        }
    }

    #[test]
    fn non_bidi_format_chars_preserved() {
        // Zero-width joiner / non-joiner are format-class but not in the
        // bidi-override range — preserved for legitimate glyph composition.
        assert_eq!(for_terminal("a\u{200D}b"), "a\u{200D}b");
    }

    #[test]
    fn empty_string_round_trips() {
        assert_eq!(for_terminal(""), "");
        assert_eq!(for_emit(""), "");
    }

    #[test]
    fn for_emit_matches_for_terminal() {
        // Documented as separate function names for callsite intent;
        // current implementations are identical. If they diverge in
        // future, this test pins the contract change.
        let samples = [
            "plain",
            "with\x1bescape",
            "bidi\u{202E}spoof",
            "日本語",
        ];
        for s in samples {
            assert_eq!(for_terminal(s), for_emit(s), "diverged on: {s:?}");
        }
    }
}
