//! SKILL.md frontmatter: the `---`-delimited YAML block at the top of the file, read as
//! top-level `key: scalar` fields. Intake is liberal (`docs/spec/skills/model.md`
//! § Frontmatter): a value ACE cannot read as YAML is kept as written, never an error.

/// Top-level scalar fields of one SKILL.md frontmatter block, in file order.
#[derive(Debug, Default)]
pub struct Frontmatter {
    fields: Vec<(String, String)>,
}

impl Frontmatter {
    /// Value of the first top-level field named `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Parse the frontmatter block of SKILL.md `content`. `None` when the content does not
/// open with a `---`-delimited block.
pub fn parse(content: &str) -> Option<Frontmatter> {
    let rest = content.trim_start().strip_prefix("---")?;
    let close = rest.find("\n---")?;
    let block = &rest[..close];

    let mut fields = Vec::new();
    let mut lines = block.lines().peekable();
    while let Some(line) = lines.next() {
        let body: Vec<&str> =
            std::iter::from_fn(|| lines.next_if(|l| is_continuation(l))).collect();
        if let Some((key, header)) = key_line(line) {
            fields.push((key.to_string(), scalar(header, &body)));
        }
    }

    Some(Frontmatter { fields })
}

/// A line that belongs to the value above it: blank, or indented.
fn is_continuation(line: &str) -> bool {
    line.is_empty() || line.starts_with([' ', '\t'])
}

/// Split a top-level `key: value` line. Comment lines and lines without a `:` are not
/// fields.
fn key_line(line: &str) -> Option<(&str, &str)> {
    if is_continuation(line) || line.starts_with('#') {
        return None;
    }
    let (key, header) = line.split_once(':')?;
    Some((key.trim(), header.trim()))
}

/// The value of a field whose first line holds `header` and whose continuation lines are
/// `body`.
fn scalar(header: &str, body: &[&str]) -> String {
    match BlockHeader::parse(header) {
        Some(block) => block.read(body),
        None => flow(header, body),
    }
}

// Flow scalars: plain or quoted, possibly folded over several lines.

/// A plain or quoted value: lines fold into one, a blank line becomes a line break.
fn flow(header: &str, body: &[&str]) -> String {
    let lines = std::iter::once(header)
        .chain(body.iter().copied())
        .map(str::trim);

    let mut folded = String::new();
    let mut breaks = 0;
    for line in lines {
        if line.is_empty() {
            breaks += 1;
            continue;
        }
        if !folded.is_empty() {
            let separator = if breaks == 0 {
                " ".to_string()
            } else {
                "\n".repeat(breaks)
            };
            folded.push_str(&separator);
        }
        folded.push_str(line);
        breaks = 0;
    }

    unquote(&folded)
}

/// Strip one pair of matching outer quotes, undoing the escapes each style needs for its
/// own delimiter: `''` in single quotes; `\"` and `\\` in double quotes. Other
/// double-quoted escapes stay as written.
fn unquote(value: &str) -> String {
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return value.to_string();
    };

    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match (c, chars.clone().next()) {
            ('\\', Some(next @ ('"' | '\\'))) => {
                out.push(next);
                chars.next();
            }
            _ => out.push(c),
        }
    }
    out
}

// Block scalars: `|` (literal) and `>` (folded), with optional chomping and indentation
// indicators in either order and an optional trailing comment.

#[derive(Debug, Clone, Copy)]
enum Style {
    Literal,
    Folded,
}

#[derive(Debug, Clone, Copy)]
enum Chomp {
    /// No indicator: one final line break.
    Clip,
    /// `-`: no final line break.
    Strip,
    /// `+`: the final line break and every trailing blank line.
    Keep,
}

#[derive(Debug)]
struct BlockHeader {
    style: Style,
    chomp: Chomp,
    /// Explicit indentation indicator; detected from the first content line when absent.
    indent: Option<usize>,
}

impl BlockHeader {
    /// Read a block-scalar header such as `>-`, `|2+`, or `> # note`. `None` when
    /// `header` is not one.
    fn parse(header: &str) -> Option<Self> {
        let (indicators, comment) = header.split_once([' ', '\t']).unwrap_or((header, ""));
        let comment = comment.trim_start();
        if !comment.is_empty() && !comment.starts_with('#') {
            return None;
        }

        let mut chars = indicators.chars();
        let style = match chars.next()? {
            '|' => Style::Literal,
            '>' => Style::Folded,
            _ => return None,
        };
        let mut chomp = None;
        let mut indent = None;
        for c in chars {
            match c {
                '-' if chomp.is_none() => chomp = Some(Chomp::Strip),
                '+' if chomp.is_none() => chomp = Some(Chomp::Keep),
                '1'..='9' if indent.is_none() => indent = c.to_digit(10).map(|d| d as usize),
                _ => return None,
            }
        }

        Some(BlockHeader {
            style,
            chomp: chomp.unwrap_or(Chomp::Clip),
            indent,
        })
    }

    /// The scalar's text from its continuation lines.
    fn read(&self, body: &[&str]) -> String {
        let indent = self.indent.unwrap_or_else(|| detect_indent(body));
        let lines: Vec<&str> = body.iter().map(|l| strip_indent(l, indent)).collect();
        let content_end = lines
            .iter()
            .rposition(|l| !l.is_empty())
            .map_or(0, |i| i + 1);
        let (text, trailing) = lines.split_at(content_end);

        let joined = match self.style {
            Style::Literal => text.join("\n"),
            Style::Folded => fold_block(text),
        };
        let has_text = usize::from(!text.is_empty());
        let final_breaks = match self.chomp {
            Chomp::Strip => 0,
            Chomp::Clip => has_text,
            Chomp::Keep => has_text + trailing.len(),
        };
        joined + &"\n".repeat(final_breaks)
    }
}

/// Indentation of the first content line; with none, the widest blank line.
fn detect_indent(body: &[&str]) -> usize {
    let first_content = body.iter().find(|l| !l.trim().is_empty());
    let widest_blank = || body.iter().map(|l| l.len()).max().unwrap_or(0);
    first_content.map_or_else(widest_blank, |l| leading_spaces(l))
}

fn leading_spaces(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// A line without its block indentation. A line of only spaces no wider than the
/// indentation is empty; one less indented loses what indentation it has.
fn strip_indent(line: &str, indent: usize) -> &str {
    &line[leading_spaces(line).min(indent)..]
}

/// Fold block-scalar lines (indentation already stripped): a single break between two
/// normal lines becomes a space, and each blank line between them a line break. Breaks
/// next to a more-indented line are kept.
fn fold_block(text: &[&str]) -> String {
    let mut out = String::new();
    let mut prev: Option<&str> = None;
    let mut blanks = 0;
    for line in text {
        if line.is_empty() {
            blanks += 1;
            continue;
        }
        out.push_str(&fold_separator(prev, line, blanks));
        out.push_str(line);
        prev = Some(line);
        blanks = 0;
    }
    out
}

/// What the line breaks before `line` fold into, given the previous content line and
/// the blank lines between them.
fn fold_separator(prev: Option<&str>, line: &str, blanks: usize) -> String {
    let more_indented = |l: &str| l.starts_with([' ', '\t']);
    match prev {
        None => "\n".repeat(blanks),
        Some(p) if more_indented(p) || more_indented(line) => "\n".repeat(blanks + 1),
        Some(_) if blanks == 0 => " ".to_string(),
        Some(_) => "\n".repeat(blanks),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(frontmatter: &str, key: &str) -> Option<String> {
        let content = format!("---\n{frontmatter}\n---\n# Body\n");
        let parsed = parse(&content).expect("content has a frontmatter block");
        parsed.get(key).map(String::from)
    }

    fn description(frontmatter: &str) -> String {
        field(frontmatter, "description").expect("description field present")
    }

    #[test]
    fn no_frontmatter_block() {
        assert!(parse("# Just a body").is_none());
        assert!(parse("---\nname: unterminated\n").is_none());
    }

    #[test]
    fn plain_and_quoted_values() {
        assert_eq!(field("name: plain", "name").as_deref(), Some("plain"));
        assert_eq!(field("name: 'it''s'", "name").as_deref(), Some("it's"));
        assert_eq!(
            field(r#"name: "say \"hi\" \\o/""#, "name").as_deref(),
            Some(r#"say "hi" \o/"#)
        );
    }

    #[test]
    fn plain_value_folds_over_indented_lines() {
        assert_eq!(
            description("description: one\n  two\n\n  three"),
            "one two\nthree"
        );
    }

    #[test]
    fn folded_clip_keeps_one_final_break() {
        assert_eq!(
            description("description: >\n  First line\n  second line."),
            "First line second line.\n"
        );
    }

    #[test]
    fn folded_strip_drops_final_break() {
        assert_eq!(
            description("description: >-\n  First line\n  second line."),
            "First line second line."
        );
    }

    #[test]
    fn folded_keep_keeps_trailing_blank_lines() {
        assert_eq!(
            description("description: >+\n  text\n\n\nname: x"),
            "text\n\n\n"
        );
    }

    #[test]
    fn folded_blank_line_is_a_line_break() {
        assert_eq!(
            description("description: >-\n  one\n  two\n\n  three"),
            "one two\nthree"
        );
    }

    #[test]
    fn folded_more_indented_lines_keep_their_breaks() {
        assert_eq!(
            description("description: >\n  one\n    code\n  two"),
            "one\n  code\ntwo\n"
        );
    }

    #[test]
    fn literal_keeps_line_breaks() {
        assert_eq!(description("description: |\n  one\n  two"), "one\ntwo\n");
        assert_eq!(description("description: |-\n  one\n  two\n\n"), "one\ntwo");
        assert_eq!(description("description: |+\n  one\n\nname: x"), "one\n\n");
    }

    #[test]
    fn explicit_indentation_in_either_order() {
        let expected = "  lead\nrest";
        assert_eq!(description("description: |2-\n    lead\n  rest"), expected);
        assert_eq!(description("description: |-2\n    lead\n  rest"), expected);
    }

    #[test]
    fn header_comment_is_ignored() {
        assert_eq!(
            description("description: >- # summary\n  one\n  two"),
            "one two"
        );
    }

    #[test]
    fn invalid_header_kept_as_written() {
        assert_eq!(description("description: >x"), ">x");
        assert_eq!(description("description: |--"), "|--");
    }

    #[test]
    fn key_like_lines_inside_values_are_not_fields() {
        let frontmatter = "description: |\n  internal: true\nmetadata:\n  name: nested\nname: real";
        assert_eq!(field(frontmatter, "internal"), None);
        assert_eq!(field(frontmatter, "name").as_deref(), Some("real"));
    }

    #[test]
    fn empty_block_scalar() {
        assert_eq!(description("description: >-\nname: x"), "");
        assert_eq!(description("description: |\nname: x"), "");
    }
}
