pub mod builtins;
mod parser;
pub mod session;

use std::collections::HashMap;

use parser::Parser;

/// Parsed template — borrows from the input string. Parse once, then call
/// `placeholders()` or `substitute()` without re-parsing.
pub struct Template<'a> {
    segments: Vec<Segment<'a>>,
    names: Vec<&'a str>,
}

#[cfg_attr(test, derive(Debug, PartialEq))]
enum Segment<'a> {
    Literal(&'a str),
    Placeholder(&'a str),
}

impl<'a> Template<'a> {
    fn new() -> Self {
        Self { segments: Vec::new(), names: Vec::new() }
    }

    /// Parse a template string into segments. Single-pass, zero-copy for literals.
    pub fn parse(input: &'a str) -> Self {
        let mut tpl = Self::new();
        Parser::new().parse_all(input, &mut tpl);
        tpl
    }

    /// Unique placeholder names in order of first appearance.
    pub fn placeholders(&self) -> &[&'a str] {
        &self.names
    }

    /// Replace placeholders with values from the map. Missing keys resolve to empty string.
    pub fn substitute(&self, values: &HashMap<String, String>) -> String {
        let mut out = String::new();
        for seg in &self.segments {
            match seg {
                Segment::Literal(s) => out.push_str(s),
                Segment::Placeholder(name) => {
                    let v = values.get(*name).map(|s| s.as_str()).unwrap_or("");
                    out.push_str(v);
                }
            }
        }
        out
    }

    pub(crate) fn push_literal(&mut self, text: &'a str) {
        if !text.is_empty() {
            self.segments.push(Segment::Literal(text));
        }
    }

    pub(crate) fn push_placeholder(&mut self, name: &'a str) {
        self.segments.push(Segment::Placeholder(name));
        if !self.names.contains(&name) {
            self.names.push(name);
        }
    }
}

/// One placeholder name in a template that does not match any allowed name,
/// optionally paired with a Levenshtein-near suggestion from the allowed set.
pub struct UnknownPlaceholder {
    pub name: String,
    pub suggestion: Option<String>,
}

/// Parse `input` and report any placeholder names not in `allowed`. Each
/// unknown carries a did-you-mean suggestion when one of the allowed names
/// is within Levenshtein distance 2.
pub fn check(input: &str, allowed: &[&str]) -> Vec<UnknownPlaceholder> {
    Template::parse(input)
        .placeholders()
        .iter()
        .filter(|name| !allowed.contains(name))
        .map(|name| UnknownPlaceholder {
            name: (*name).to_string(),
            suggestion: nearest(name, allowed),
        })
        .collect()
}

fn nearest(name: &str, allowed: &[&str]) -> Option<String> {
    allowed
        .iter()
        .map(|cand| (*cand, levenshtein(name, cand)))
        .filter(|(_, d)| *d <= 2)
        .min_by_key(|(_, d)| *d)
        .map(|(cand, _)| cand.to_string())
}

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (n, m) = (a.len(), b.len());
    if n == 0 { return m; }
    if m == 0 { return n; }

    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr = vec![0usize; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (curr[j - 1] + 1)
                .min(prev[j] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vals(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    // -- substitute (data-driven) --

    #[test]
    fn substitute_cases() {
        type Case<'a> = (&'a str, &'a [(&'a str, &'a str)], &'a str);
        let cases: &[Case] = &[
            // basic
            ("Hello {{ name }}!", &[("name", "world")], "Hello world!"),
            ("{{x}}", &[("x", "1")], "1"),
            ("{{  key  }}", &[("key", "val")], "val"),
            ("{{ a }}+{{ b }}", &[("a", "1"), ("b", "2")], "1+2"),
            ("{{a}}{{b}}", &[("a", "x"), ("b", "y")], "xy"),

            // missing key → empty
            ("{{ missing }}", &[], ""),
            ("[{{ gone }}]", &[], "[]"),

            // no placeholders → passthrough
            ("no placeholders here", &[("a", "b")], "no placeholders here"),
            ("", &[], ""),

            // single braces → not a placeholder
            ("{x}", &[("x", "1")], "{x}"),
            ("{x} and {{ x }}", &[("x", "1")], "{x} and 1"),

            // invalid names → preserved as literal
            ("{{ not-valid }}", &[], "{{ not-valid }}"),
            ("{{ has space }}", &[], "{{ has space }}"),
            ("{{ 123.456 }}", &[], "{{ 123.456 }}"),
            ("{{ a!b }}", &[], "{{ a!b }}"),

            // empty braces → literal
            ("{{}}", &[], "{{}}"),
            ("{{ }}", &[], "{{ }}"),

            // unbalanced / broken open braces
            ("end{", &[], "end{"),
            ("end{{", &[], "end{{"),
            ("{", &[], "{"),
            ("{{", &[], "{{"),
            ("{{{", &[], "{{{"),
            ("a{b", &[], "a{b"),
            ("a{{b", &[], "a{{b"),

            // unbalanced / broken close braces
            ("a}b", &[], "a}b"),
            ("a}}b", &[], "a}}b"),
            ("}}", &[], "}}"),
            ("}}}", &[], "}}}"),

            // incomplete placeholder (no closing)
            ("{{ name", &[], "{{ name"),
            ("{{ name }", &[], "{{ name }"),
            ("before {{ name", &[], "before {{ name"),

            // triple braces — inner `{` becomes part of name (invalid), all literal
            ("{{{ x }}}", &[("x", "v")], "{{{ x }}}"),
            ("{{{{ x }}}}", &[("x", "v")], "{{{{ x }}}}"),

            // mixed valid and broken
            ("{{ a }} {{ bad- }} {{ b }}", &[("a", "1"), ("b", "2")], "1 {{ bad- }} 2"),
            ("ok {{ x }} {{ }} tail", &[("x", "v")], "ok v {{ }} tail"),

            // newlines in and around placeholders
            ("line1\n{{ x }}\nline2", &[("x", "mid")], "line1\nmid\nline2"),
            ("{{ x\n}}", &[], "{{ x\n}}"),
            ("\n\n{{ a }}\n\n", &[("a", "b")], "\n\nb\n\n"),

            // unicode passthrough
            ("héllo {{ name }} wörld", &[("name", "日本")], "héllo 日本 wörld"),
            ("{{ emoji }}", &[("emoji", "🎉")], "🎉"),
            ("café ☕ {{ x }}", &[("x", "✓")], "café ☕ ✓"),

            // broken unicode-like sequences (not actual broken UTF-8, just unusual chars)
            ("{{ naïve }}", &[], "{{ naïve }}"),
            ("curly \u{201c}quotes\u{201d} {{ x }}", &[("x", "v")], "curly \u{201c}quotes\u{201d} v"),

            // real-world: MCP header
            ("Bearer {{ github_pat }}", &[("github_pat", "ghp_abc")], "Bearer ghp_abc"),
        ];

        for (i, (input, pairs, expected)) in cases.iter().enumerate() {
            let tpl = Template::parse(input);
            let result = tpl.substitute(&vals(pairs));
            assert_eq!(&result, expected, "case {i}: input={input:?}");
        }
    }

    // -- placeholders (data-driven) --

    #[test]
    fn placeholders_cases() {
        let cases: &[(&str, &[&str])] = &[
            ("Hello {{ name }}!", &["name"]),
            ("{{ a }} and {{ b }} and {{ a }}", &["a", "b"]),
            ("plain text", &[]),
            ("{{name}} and {{  spaced  }}", &["name", "spaced"]),
            ("{{ not-valid }} {{ ok_1 }}", &["ok_1"]),
            ("{{}} and {{ }}", &[]),
            ("{{ a }} {{ bad- }} {{ b }}", &["a", "b"]),
            ("{{}}{{{x}}}", &[]),
            ("{{ name", &[]),
            ("{", &[]),
            ("", &[]),
        ];

        for (i, (input, expected)) in cases.iter().enumerate() {
            let tpl = Template::parse(input);
            assert_eq!(tpl.placeholders(), *expected, "case {i}: input={input:?}");
        }
    }

    // -- parse structure --

    #[test]
    fn parse_mixed_segments() {
        let tpl = Template::parse("hi {{ name }}, welcome");
        assert_eq!(tpl.segments, vec![
            Segment::Literal("hi "),
            Segment::Placeholder("name"),
            Segment::Literal(", welcome"),
        ]);
    }

    // -- check (data-driven) --

    #[test]
    fn check_cases() {
        let allowed = &["school_dir", "project_dir", "home", "backend_dir"];
        type Case<'a> = (&'a str, &'a [(&'a str, Option<&'a str>)]);
        let cases: &[Case] = &[
            // clean
            ("", &[]),
            ("plain literal", &[]),
            ("{{ school_dir }}/x", &[]),
            ("{{ school_dir }} {{ project_dir }} {{ home }} {{ backend_dir }}", &[]),

            // single typo with suggestion
            ("{{ schol_dir }}/x", &[("schol_dir", Some("school_dir"))]),
            ("{{ projectdir }}/x", &[("projectdir", Some("project_dir"))]),
            ("{{ hom }}", &[("hom", Some("home"))]),

            // multiple typos
            (
                "{{ schol_dir }}/{{ projct_dir }}",
                &[
                    ("schol_dir", Some("school_dir")),
                    ("projct_dir", Some("project_dir")),
                ],
            ),

            // mixed valid + typo: only typo flagged
            ("{{ school_dir }}/{{ schol_dir }}", &[("schol_dir", Some("school_dir"))]),

            // unknown with no near match
            ("{{ totally_different }}", &[("totally_different", None)]),

            // broken placeholder syntax → parser drops it → no issue
            ("{{ }} {{ bad- }} {{ has space }}", &[]),
            ("{{ schol_dir", &[]),
            ("{schol_dir}", &[]),

            // duplicates collapse (placeholders() returns unique names)
            ("{{ schol_dir }} and {{ schol_dir }}", &[("schol_dir", Some("school_dir"))]),
        ];

        for (i, (input, expected)) in cases.iter().enumerate() {
            let issues = check(input, allowed);
            let got: Vec<(&str, Option<&str>)> = issues
                .iter()
                .map(|u| (u.name.as_str(), u.suggestion.as_deref()))
                .collect();
            let want: Vec<(&str, Option<&str>)> = expected.to_vec();
            assert_eq!(got, want, "case {i}: input={input:?}");
        }
    }

    #[test]
    fn parse_reuse() {
        let tpl = Template::parse("{{ x }} and {{ y }}");
        let v1 = tpl.substitute(&vals(&[("x", "a"), ("y", "b")]));
        let v2 = tpl.substitute(&vals(&[("x", "1"), ("y", "2")]));
        assert_eq!(v1, "a and b");
        assert_eq!(v2, "1 and 2");
    }
}
