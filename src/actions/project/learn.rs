//! `ace learn` — study the project, edit the instructions file in place,
//! and narrow the project's `skills` filter.
//!
//! See `spec/learn.md` for the full design. The action one-shots the
//! backend with the LEARN prompt; the agent edits its own instructions
//! file and emits skill names/globs on stdout. ACE parses forgivingly
//! and rewrites `ace.toml`'s `skills` array.

use std::collections::{HashMap, HashSet};
use std::process::ExitStatus;

use crate::ace::{Ace, IoError};
use crate::backend::{BackendError, OneShotRequest, PromptInput};
use crate::config::{ace_toml, ConfigError};
use crate::school::SchoolError;
use crate::skills::SkillError;
use crate::templates::{self, Template};

#[derive(Debug, thiserror::Error)]
pub enum LearnError {
    #[error("{0}")]
    School(#[from] SchoolError),
    #[error("{0}")]
    Skill(#[from] SkillError),
    #[error("{0}")]
    Config(#[from] ConfigError),
    #[error("{0}")]
    Backend(#[from] BackendError),
    #[error("{0}")]
    Prompt(#[from] IoError),
    #[error("backend spawn failed: {0}")]
    BackendSpawn(std::io::Error),
    #[error("backend exited {status}: {stderr}")]
    BackendNonZero { status: ExitStatus, stderr: String },
    #[error("write ace.toml: {0}")]
    TomlWrite(std::io::Error),
}

/// Pure work — study, parse, write. Callers own the user-confirm step;
/// see `cmd/learn.rs` (explicit invocation) and
/// `school::skill_count::maybe_offer_learn` (auto-trigger).
pub struct LearnAction;

impl LearnAction {
    pub fn run(&self, ace: &mut Ace) -> Result<(), LearnError> {
        // Resolve school first (errors with `School` if missing).
        ace.require_school()?;

        // Build {available_skills} from the school's skill index.
        let available: Vec<String> = ace.skills()?.iter().map(|s| s.name.clone()).collect();
        let prompt_text = render_prompt(&available);

        let project_dir = ace.project_dir().to_path_buf();
        let env: HashMap<String, String> = ace
            .require_resolved()?
            .env
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect();
        let backend = ace.backend()?.clone();

        ace.progress("Studying project — this may take a minute");

        let output = backend
            .exec_one_shot(OneShotRequest {
                prompt: PromptInput::Inline(prompt_text),
                project_dir,
                env,
                extra_args: Vec::new(),
            })
            .map_err(LearnError::BackendSpawn)?;

        if !output.status.success() {
            return Err(LearnError::BackendNonZero {
                status: output.status,
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let known: HashSet<&str> = available.iter().map(String::as_str).collect();
        let parsed = parse_stdout(&stdout, &known);

        for warn in &parsed.warnings {
            ace.warn(warn);
        }
        ace.done(&format!(
            "kept {} entries, dropped {} lines",
            parsed.kept.len(),
            parsed.warnings.len(),
        ));

        // Write parsed list into ace.toml. Distinguish io errors from other
        // ConfigError variants so the caller sees a useful TomlWrite signal.
        let path = ace.project_dir().join("ace.toml");
        let mut config = ace_toml::load_or_default(&path)?;
        config.skills = parsed.kept;
        ace_toml::save(&path, &config).map_err(|e| match e {
            ConfigError::Io(io) => LearnError::TomlWrite(io),
            other => LearnError::Config(other),
        })?;

        Ok(())
    }
}

fn render_prompt(available: &[String]) -> String {
    let tpl = Template::parse(templates::builtins::LEARN);
    let values = HashMap::from([
        ("available_skills".to_string(), available.join("\n")),
    ]);
    tpl.substitute(&values)
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ParseResult {
    pub kept: Vec<String>,
    pub warnings: Vec<String>,
}

/// Parse the agent's stdout into a final skills list.
///
/// Forgiving — LLMs hallucinate. Per line: trim, strip stray decoration
/// (bullets, backticks, fence markers, trailing punctuation). Blanks are
/// skipped. The residue is kept if it's a valid glob pattern OR a literal
/// name in `known`. Otherwise dropped with a warning naming the reason.
///
/// Order is preserved; duplicates are de-duped (first occurrence wins).
pub(crate) fn parse_stdout(stdout: &str, known: &HashSet<&str>) -> ParseResult {
    let mut result = ParseResult::default();
    let mut seen: HashSet<String> = HashSet::new();

    for line in stdout.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip fence markers entirely (don't treat as content).
        if is_fence_marker(trimmed) {
            continue;
        }

        let stripped = strip_decoration(trimmed);
        if stripped.is_empty() {
            continue;
        }

        if !looks_like_skill_token(&stripped) {
            result.warnings.push(format!(
                "ace learn: dropped {trimmed:?} (looks like prose)"
            ));
            continue;
        }

        let is_glob = is_glob_pattern(&stripped);
        let is_known = known.contains(stripped.as_str());

        if !is_glob && !is_known {
            result.warnings.push(format!(
                "ace learn: dropped {stripped:?} (unknown skill)"
            ));
            continue;
        }

        if seen.insert(stripped.clone()) {
            result.kept.push(stripped);
        }
    }

    result
}

fn is_fence_marker(s: &str) -> bool {
    s.starts_with("```")
}

/// Strip leading bullet markers, backticks, and trailing punctuation.
fn strip_decoration(s: &str) -> String {
    let mut t = s;
    // Leading bullets: "- ", "* ", "• "
    for prefix in ["- ", "* ", "• "] {
        if let Some(rest) = t.strip_prefix(prefix) {
            t = rest;
            break;
        }
    }
    // Surrounding backticks: `name`
    let t = t.trim_matches('`');
    // Trailing punctuation that could leak in from prose: , . ; :
    let t = t.trim_end_matches(|c: char| matches!(c, ',' | '.' | ';' | ':'));
    t.trim().to_string()
}

/// A skill token is a name or glob made of letters/digits/`-`/`_`/`*`/`?`/`/`.
/// Spaces or other characters mean it's prose.
fn looks_like_skill_token(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '*' | '?' | '/' | '.')
        })
}

fn is_glob_pattern(s: &str) -> bool {
    s.contains('*') || s.contains('?')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known<'a>(items: &[&'a str]) -> HashSet<&'a str> {
        items.iter().copied().collect()
    }

    #[test]
    fn parses_clean_strict_form() {
        let stdout = "general-coding\nrust-coding\nsimplify\n";
        let r = parse_stdout(stdout, &known(&["general-coding", "rust-coding", "simplify"]));
        assert_eq!(r.kept, vec!["general-coding", "rust-coding", "simplify"]);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn keeps_glob_patterns_without_index_check() {
        let stdout = "frontend-*\nrust-coding\n";
        let r = parse_stdout(stdout, &known(&["rust-coding"]));
        assert_eq!(r.kept, vec!["frontend-*", "rust-coding"]);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn drops_unknown_literal_with_warning() {
        let stdout = "rust-coding\nbogus-skill\n";
        let r = parse_stdout(stdout, &known(&["rust-coding"]));
        assert_eq!(r.kept, vec!["rust-coding"]);
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("bogus-skill"));
        assert!(r.warnings[0].contains("unknown skill"));
    }

    #[test]
    fn strips_bullet_decoration() {
        let stdout = "- rust-coding\n* simplify\n";
        let r = parse_stdout(stdout, &known(&["rust-coding", "simplify"]));
        assert_eq!(r.kept, vec!["rust-coding", "simplify"]);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn strips_backticks() {
        let stdout = "`rust-coding`\n";
        let r = parse_stdout(stdout, &known(&["rust-coding"]));
        assert_eq!(r.kept, vec!["rust-coding"]);
    }

    #[test]
    fn ignores_fence_markers() {
        let stdout = "```\nrust-coding\n```\n";
        let r = parse_stdout(stdout, &known(&["rust-coding"]));
        assert_eq!(r.kept, vec!["rust-coding"]);
        assert!(r.warnings.is_empty(), "fence lines should be silently skipped");
    }

    #[test]
    fn ignores_blank_lines() {
        let stdout = "\nrust-coding\n\n\nsimplify\n";
        let r = parse_stdout(stdout, &known(&["rust-coding", "simplify"]));
        assert_eq!(r.kept, vec!["rust-coding", "simplify"]);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn drops_prose_lines_with_warning() {
        let stdout = "Here are the skills:\nrust-coding\n";
        let r = parse_stdout(stdout, &known(&["rust-coding"]));
        assert_eq!(r.kept, vec!["rust-coding"]);
        assert_eq!(r.warnings.len(), 1);
        assert!(r.warnings[0].contains("looks like prose"));
    }

    #[test]
    fn dedupes_repeats() {
        let stdout = "rust-coding\nrust-coding\nsimplify\nrust-coding\n";
        let r = parse_stdout(stdout, &known(&["rust-coding", "simplify"]));
        assert_eq!(r.kept, vec!["rust-coding", "simplify"]);
    }

    #[test]
    fn strips_trailing_punctuation() {
        let stdout = "rust-coding,\nsimplify.\n";
        let r = parse_stdout(stdout, &known(&["rust-coding", "simplify"]));
        assert_eq!(r.kept, vec!["rust-coding", "simplify"]);
    }

    #[test]
    fn empty_stdout_returns_empty_kept() {
        let r = parse_stdout("", &known(&["any"]));
        assert!(r.kept.is_empty());
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn glob_question_mark_kept() {
        let stdout = "frontend-?\n";
        let r = parse_stdout(stdout, &known(&[]));
        assert_eq!(r.kept, vec!["frontend-?"]);
    }
}
