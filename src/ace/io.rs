use std::collections::HashSet;
use std::io::{IsTerminal, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

// ANSI escape sequences for terminal state management.
// Alt screen is a separate buffer that preserves the user's scrollback.
// Cursor hide/show prevents a flickering cursor during full-screen redraws.
// Alt-screen entry currently has no caller. Kept, with its plumbing, for
// full-screen UI work that is planned but not yet written.
#[allow(dead_code)]
const ENTER_ALT_SCREEN: &[u8] = b"\x1b[?1049h"; // switch to alt screen buffer
#[allow(dead_code)]
const HIDE_CURSOR: &[u8] = b"\x1b[?25l";

const CLEANUP_CURSOR: &[u8] = b"\x1b[?25h";
const CLEANUP_ALT_SCREEN: &[u8] = b"\x1b[?1049l\x1b[?25h";

fn cleanup_bytes_for(alt_screen: bool) -> &'static [u8] {
    if alt_screen {
        CLEANUP_ALT_SCREEN
    } else {
        CLEANUP_CURSOR
    }
}

// Global alt-screen flag shared with the process-wide Ctrl+C handler.
// `ctrlc::set_handler` can only be called once per process, so we register
// once via `OnceLock` and let each `TerminalGuard` mutate this flag.
fn alt_screen_flag() -> &'static Arc<AtomicBool> {
    static FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    FLAG.get_or_init(|| {
        let flag = Arc::new(AtomicBool::new(false));
        let handler_flag = Arc::clone(&flag);
        if let Err(e) = ctrlc::set_handler(move || {
            let cleanup = cleanup_bytes_for(handler_flag.load(Ordering::Relaxed));
            let _ = std::io::stderr().write_all(cleanup);
            let _ = std::io::stderr().flush();
            std::process::exit(130);
        }) {
            eprintln!("warning: failed to register Ctrl+C handler: {e}");
        }
        flag
    })
}

/// RAII guard that restores terminal state on drop and on SIGINT.
///
/// Starts in cursor-restore mode (show cursor only). Call `enter_alt_screen()`
/// to upgrade — both drop and SIGINT will then also exit the alternate screen.
///
/// Registers a process-wide Ctrl+C handler exactly once (ctrlc crate wraps
/// `SetConsoleCtrlHandler` on Windows and `sigaction` on Unix). The handler
/// reads a shared atomic flag so it always sees the current alt-screen mode.
pub struct TerminalGuard {
    alt_screen: Arc<AtomicBool>,
}

impl TerminalGuard {
    pub fn new() -> Self {
        let alt_screen = Arc::clone(alt_screen_flag());
        alt_screen.store(false, Ordering::Relaxed);
        Self { alt_screen }
    }

    /// Upgrade to alt-screen mode. Both drop and SIGINT will exit the
    /// alternate screen buffer in addition to restoring the cursor.
    #[allow(dead_code)]
    pub fn enter_alt_screen(&self) {
        self.alt_screen.store(true, Ordering::Relaxed);
    }

    fn cleanup_bytes(&self) -> &'static [u8] {
        cleanup_bytes_for(self.alt_screen.load(Ordering::Relaxed))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = std::io::stderr().write_all(self.cleanup_bytes());
        let _ = std::io::stderr().flush();
        self.alt_screen.store(false, Ordering::Relaxed);
    }
}

/// Environment variables that mark an unattended run. Any one of them present
/// and non-empty means nobody is watching, so ACE must not wait on an answer.
const CI_VARS: [&str; 2] = ["CI", "CONTINUOUS_INTEGRATION"];

fn in_ci() -> bool {
    CI_VARS
        .iter()
        .any(|key| std::env::var_os(key).is_some_and(|v| !v.is_empty()))
}

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("cancelled")]
    Cancelled,
    #[error("no terminal to answer \"{prompt}\"")]
    NoTerminal { prompt: String },
    #[error("nothing may answer \"{prompt}\" — asking was waived")]
    AskingWaived { prompt: String },
    #[error("nothing may answer \"{prompt}\" — output is machine-readable")]
    MachineReadable { prompt: String },
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl IoError {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::NoTerminal { .. } => {
                Some("run this in an interactive terminal, or set the value with `ace config set`")
            }
            Self::AskingWaived { .. } => {
                Some("drop `--yes` (or unset CI) to answer, or set the value with `ace config set`")
            }
            Self::MachineReadable { .. } => {
                Some("drop `--porcelain` to answer, or set the value with `ace config set`")
            }
            Self::Cancelled | Self::Io(_) => None,
        }
    }
}

/// Owns every input that decides how ACE talks to whoever is on the other end —
/// CLI flags, environment, terminal presence — and answers the questions callers
/// actually have (`should_colorize`, `should_emit`, `can_ask`) rather than
/// exposing a mode for them to re-interpret.
pub struct Io {
    porcelain: bool,
    quiet: bool,
    yes: bool,
    ci: bool,
    is_terminal: bool,
    stdout_is_terminal: bool,
    spinner: Option<ProgressBar>,
    /// Held for its `Drop`, which restores the cursor; read only by
    /// `enter_alt_screen`, which is idle until full-screen UI lands.
    #[allow(dead_code)]
    guard: Option<TerminalGuard>,
}

const BIG_LOGO: &str = concat!(
    "\x1b[1;38;2;55;225;225m╭──╮  ",
    "\x1b[38;2;30;205;230m╭───  ",
    "\x1b[38;2;40;175;225m╭───\x1b[0m\n",
    "\x1b[1;38;2;55;225;225m│  │  ",
    "\x1b[38;2;30;205;230m│     ",
    "\x1b[38;2;40;175;225m│ ──\x1b[0m\n",
    "\x1b[1;38;2;55;225;225m╵  ╵  ",
    "\x1b[38;2;30;205;230m╰───  ",
    "\x1b[38;2;40;175;225m╰───\x1b[0m",
);

const COMPACT_LOGO: &str = concat!(
    "\x1b[1;38;2;55;225;225mΠ",
    "\x1b[38;2;30;205;230mC",
    "\x1b[38;2;40;175;225mE\x1b[0m",
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordmarkStyle {
    Big,
    Compact,
    None,
}

impl Io {
    pub fn new(porcelain: bool, yes: bool) -> Self {
        let is_terminal = std::io::stderr().is_terminal();
        let guard = (!porcelain && is_terminal).then(TerminalGuard::new);

        Self {
            porcelain,
            quiet: false,
            yes,
            ci: in_ci(),
            is_terminal,
            stdout_is_terminal: std::io::stdout().is_terminal(),
            spinner: None,
            guard,
        }
    }

    /// Suppress all output for the rest of the run. Used by the background
    /// upgrade spawn, which has no one to talk to.
    pub fn silence(&mut self) {
        self.quiet = true;
    }

    // -- what the caller actually wants to know --

    /// Decoration, color, spinners, and the logo. Needs both a terminal to
    /// render into and the user's consent to spend it on presentation.
    pub fn should_colorize(&self) -> bool {
        !self.porcelain && self.is_terminal
    }

    pub fn should_emit(&self) -> bool {
        !self.quiet
    }

    /// Long data output goes through a pager only when a human is reading it
    /// on a terminal — machine-readable and piped output must stay a plain
    /// stream. Keyed on stdout, where data lands, not stderr.
    pub fn should_page(&self) -> bool {
        !self.porcelain && self.stdout_is_terminal
    }

    /// Whether a question can reach a human who will answer it: someone has to
    /// be there, they must not have waived being asked, and the output must not
    /// be addressed to a machine — `--porcelain` means something is parsing
    /// this, and a prompt would be a hang rather than a question.
    pub fn can_ask(&self) -> bool {
        self.is_terminal && !self.porcelain && !self.yes && !self.ci
    }

    pub fn logo(&self, wordmark: WordmarkStyle) -> &'static str {
        if !self.should_colorize() || !self.should_emit() {
            return "";
        }

        match wordmark {
            WordmarkStyle::Big => BIG_LOGO,
            WordmarkStyle::Compact => COMPACT_LOGO,
            WordmarkStyle::None => "",
        }
    }

    /// Enter alternate screen buffer. The guard will exit it on drop/SIGINT.
    /// No-op in Porcelain/Silent mode (no terminal to manage).
    #[allow(dead_code)]
    pub fn enter_alt_screen(&self) {
        if let Some(guard) = &self.guard {
            guard.enter_alt_screen();
            let _ = std::io::stderr().write_all(ENTER_ALT_SCREEN);
            let _ = std::io::stderr().write_all(HIDE_CURSOR);
            let _ = std::io::stderr().flush();
        }
    }

    // -- output --

    pub fn progress(&mut self, msg: &str) {
        self.clear_spinner();
        if !self.should_colorize() || !self.should_emit() {
            return;
        }
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner} {msg}")
                .expect("valid template"),
        );
        pb.set_message(msg.to_string());
        pb.enable_steady_tick(std::time::Duration::from_millis(80));
        self.spinner = Some(pb);
    }

    pub fn done(&mut self, msg: &str) {
        self.line(&format!("{} {msg}", style("✓").green()), msg);
    }

    pub fn info(&mut self, msg: &str) {
        self.line(&format!("  {msg}"), msg);
    }

    pub fn warn(&mut self, msg: &str) {
        self.line(&format!("{} {msg}", style("⚠").yellow()), msg);
    }

    pub fn error(&mut self, msg: &str) {
        self.line(&format!("{} {msg}", style("✗").red()), msg);
    }

    pub fn hint(&mut self, msg: &str) {
        self.line(
            &format!("  {} {msg}", style("→").dim()),
            &format!("hint: {msg}"),
        );
    }

    /// One status line on stderr, in whichever dress the caller's environment
    /// asked for.
    fn line(&mut self, decorated: &str, plain: &str) {
        self.clear_spinner();
        if !self.should_emit() {
            return;
        }
        if self.should_colorize() {
            eprintln!("{decorated}");
        } else {
            eprintln!("{plain}");
        }
    }

    pub fn data(&mut self, msg: &str) {
        self.clear_spinner();
        if !self.should_emit() {
            return;
        }
        println!("{msg}");
    }

    /// Emit potentially-long data through `$PAGER` (default `less -FRX`, which
    /// passes short output straight through). Falls back to a plain `data`
    /// print when paging is off or the pager cannot run.
    pub fn page(&mut self, content: &str) {
        self.clear_spinner();
        if !self.should_emit() {
            return;
        }
        if !self.should_page() {
            println!("{content}");
            return;
        }

        match self.pipe_to_pager(content) {
            Ok(()) => {}
            Err(e) => {
                self.warn(&format!("pager failed, printing directly: {e}"));
                println!("{content}");
            }
        }
    }

    fn pipe_to_pager(&self, content: &str) -> std::io::Result<()> {
        let pager = std::env::var("PAGER").unwrap_or_else(|_| "less -FRX".to_string());
        let mut parts = pager.split_whitespace();
        let bin = parts.next().unwrap_or("less");

        let mut child = std::process::Command::new(bin)
            .args(parts)
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        // The temporary drops at end of statement, closing the pipe so the
        // pager sees EOF. Quitting the pager mid-stream closes its stdin
        // first — that BrokenPipe is the user's "seen enough", not a failure
        // to fall back from.
        let written = child
            .stdin
            .take()
            .expect("stdin was piped")
            .write_all(content.as_bytes());
        child.wait()?;

        match written {
            Err(e) if e.kind() != std::io::ErrorKind::BrokenPipe => Err(e),
            _ => Ok(()),
        }
    }

    pub fn separator(&mut self) {
        self.clear_spinner();
        if self.should_colorize() && self.should_emit() {
            eprintln!("\n{}\n", style("⟢⟢⟢⟢⟢⟢⟢").dim());
        }
    }

    // -- input --

    pub fn prompt_text(&mut self, prompt: &str, initial: Option<&str>) -> Result<String, IoError> {
        self.require_ask(prompt)?;

        self.clear_spinner();
        let mut p = inquire::Text::new(prompt);
        if let Some(val) = initial {
            p = p.with_initial_value(val);
        }
        p.prompt().map_err(map_inquire_err)
    }

    pub fn prompt_select(&mut self, prompt: &str, options: Vec<String>) -> Result<String, IoError> {
        self.require_ask(prompt)?;

        self.clear_spinner();
        inquire::Select::new(prompt, options)
            .prompt()
            .map_err(map_inquire_err)
    }

    /// Free-form prompts have no defensible unattended answer — unlike a
    /// checklist, which falls back to all-or-none. Refuse rather than invent
    /// one on the user's behalf, naming whichever cause the caller can act on.
    fn require_ask(&self, prompt: &str) -> Result<(), IoError> {
        if self.can_ask() {
            return Ok(());
        }

        // Most fundamental cause first: dropping a flag cannot conjure a
        // terminal, so a pipe is named ahead of anything the caller passed.
        let prompt = prompt.to_string();
        if !self.is_terminal {
            return Err(IoError::NoTerminal { prompt });
        }
        if self.porcelain {
            return Err(IoError::MachineReadable { prompt });
        }
        Err(IoError::AskingWaived { prompt })
    }

    /// Present a checklist and return the indices of the ticked options.
    ///
    /// Indices rather than values so callers match picks back against their own
    /// list without a stringly round-trip. When ACE may not ask, there is no one
    /// to tick boxes, so `default_all` decides: all or none.
    pub fn prompt_multiselect(
        &mut self,
        prompt: &str,
        options: Vec<String>,
        default_all: bool,
    ) -> Result<Vec<usize>, IoError> {
        if !self.can_ask() {
            let all = (0..options.len()).collect();
            return Ok(if default_all { all } else { Vec::new() });
        }

        self.clear_spinner();
        let mut p = inquire::MultiSelect::new(prompt, options);
        if default_all {
            p = p.with_all_selected_by_default();
        }

        let picked = p.raw_prompt().map_err(map_inquire_err)?;
        Ok(picked.iter().map(|opt| opt.index).collect())
    }

    fn clear_spinner(&mut self) {
        if let Some(sp) = self.spinner.take() {
            sp.finish_and_clear();
        }
    }
}

/// Split `items` into (picked, declined) by index membership in `picked`,
/// the return of `prompt_multiselect`. Order is preserved within each half.
pub fn partition_picked<T: Clone>(items: &[T], picked: &[usize]) -> (Vec<T>, Vec<T>) {
    let picked: HashSet<usize> = picked.iter().copied().collect();

    let mut chosen = Vec::new();
    let mut declined = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if picked.contains(&i) {
            chosen.push(item.clone());
        } else {
            declined.push(item.clone());
        }
    }

    (chosen, declined)
}

fn map_inquire_err(e: inquire::InquireError) -> IoError {
    match e {
        inquire::InquireError::OperationCanceled | inquire::InquireError::OperationInterrupted => {
            IoError::Cancelled
        }
        other => IoError::Io(std::io::Error::other(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The real constructor reads the process's terminal and environment, which
    /// a test cannot vary. This one takes them as arguments instead.
    fn io_with(porcelain: bool, yes: bool, ci: bool, is_terminal: bool) -> Io {
        Io {
            porcelain,
            quiet: false,
            yes,
            ci,
            is_terminal,
            stdout_is_terminal: is_terminal,
            spinner: None,
            guard: None,
        }
    }

    fn piped() -> Io {
        io_with(false, false, false, false)
    }

    fn attended() -> Io {
        io_with(false, false, false, true)
    }

    // -- should_colorize --

    #[test]
    fn colorizes_only_for_an_unsuppressed_terminal() {
        assert!(attended().should_colorize());
        assert!(!piped().should_colorize());
        assert!(!io_with(true, false, false, true).should_colorize());
    }

    #[test]
    fn logo_uses_the_locked_big_wordmark() {
        let expected = concat!(
            "\x1b[1;38;2;55;225;225m╭──╮  ",
            "\x1b[38;2;30;205;230m╭───  ",
            "\x1b[38;2;40;175;225m╭───\x1b[0m\n",
            "\x1b[1;38;2;55;225;225m│  │  ",
            "\x1b[38;2;30;205;230m│     ",
            "\x1b[38;2;40;175;225m│ ──\x1b[0m\n",
            "\x1b[1;38;2;55;225;225m╵  ╵  ",
            "\x1b[38;2;30;205;230m╰───  ",
            "\x1b[38;2;40;175;225m╰───\x1b[0m",
        );

        assert_eq!(attended().logo(WordmarkStyle::Big), expected);
    }

    #[test]
    fn logo_uses_the_locked_compact_wordmark() {
        let expected = concat!(
            "\x1b[1;38;2;55;225;225mΠ",
            "\x1b[38;2;30;205;230mC",
            "\x1b[38;2;40;175;225mE\x1b[0m",
        );

        assert_eq!(attended().logo(WordmarkStyle::Compact), expected);
    }

    #[test]
    fn logo_suppresses_an_absent_wordmark() {
        assert_eq!(attended().logo(WordmarkStyle::None), "");
    }

    #[test]
    fn logo_suppresses_all_wordmarks_without_presentation() {
        assert_eq!(piped().logo(WordmarkStyle::Big), "");
        assert_eq!(
            io_with(true, false, false, true).logo(WordmarkStyle::Compact),
            ""
        );
    }

    // Waiving prompts is an intention; machine-readable output is an
    // environment. Answering yes in advance says nothing about how the output
    // should look.
    #[test]
    fn waiving_prompts_does_not_suppress_color() {
        assert!(io_with(false, true, false, true).should_colorize());
    }

    // -- should_page --

    #[test]
    fn pages_only_for_a_human_terminal() {
        assert!(attended().should_page());
    }

    // A pipe reads a stream; a pager in the middle would hang or garble it.
    #[test]
    fn never_pages_into_a_pipe() {
        assert!(!piped().should_page());
    }

    // Porcelain means a machine is parsing stdout even if a terminal is
    // attached — same reasoning as can_ask.
    #[test]
    fn never_pages_machine_readable_output() {
        assert!(!io_with(true, false, false, true).should_page());
    }

    // -- can_ask --

    #[test]
    fn cannot_ask_without_a_terminal() {
        assert!(!piped().can_ask());
    }

    // Machine-readable output means a machine is reading it. A prompt would be
    // a hang, and a terminal being attached does not make one a person.
    #[test]
    fn cannot_ask_when_output_is_machine_readable() {
        assert!(!io_with(true, false, false, true).can_ask());
    }

    #[test]
    fn cannot_ask_once_the_user_waives_it() {
        assert!(!io_with(false, true, false, true).can_ask());
        assert!(!io_with(false, false, true, true).can_ask());
    }

    #[test]
    fn ci_env_var_must_be_non_empty_to_count() {
        assert!(attended().can_ask());
    }

    // -- refusals --

    // A pipe has nowhere to type an answer. Substituting a default would put
    // words in the user's mouth, so both free-form prompts refuse instead.
    #[test]
    fn free_form_prompts_refuse_without_a_terminal() {
        let mut io = piped();

        let err = io
            .prompt_text("School name:", None)
            .expect_err("no terminal to answer on");

        assert!(matches!(err, IoError::NoTerminal { .. }));
        assert!(err.to_string().contains("School name:"));
        assert!(err.hint().is_some());

        let err = piped()
            .prompt_select("Pick a backend:", vec!["claude".to_string()])
            .expect_err("no terminal to answer on");
        assert!(matches!(err, IoError::NoTerminal { .. }));
    }

    // Porcelain is neither a waiver nor a missing terminal, so it gets its own
    // cause — telling this caller to drop `--yes` would name a flag they never
    // passed.
    #[test]
    fn free_form_prompts_name_porcelain_as_its_own_cause() {
        let err = io_with(true, false, false, true)
            .prompt_text("School name:", None)
            .expect_err("output is machine-readable");

        assert!(matches!(err, IoError::MachineReadable { .. }));
        assert!(err.to_string().contains("School name:"));
        assert!(
            err.hint().expect("hint").contains("--porcelain"),
            "the hint must name the flag actually in play"
        );
    }

    // A terminal exists but the user waived the question, so the error names the
    // flag they can drop rather than blaming the environment.
    #[test]
    fn free_form_prompts_name_the_waiver_when_one_exists() {
        let err = io_with(false, true, false, true)
            .prompt_text("School name:", None)
            .expect_err("asking was waived");

        assert!(matches!(err, IoError::AskingWaived { .. }));
        assert!(err.to_string().contains("School name:"));
        assert!(err.hint().is_some());

        let err = io_with(false, false, true, true)
            .prompt_select("Pick a backend:", vec!["claude".to_string()])
            .expect_err("asking was waived");
        assert!(matches!(err, IoError::AskingWaived { .. }));
    }

    // The checklist keeps its all-or-none resolution — it has a defensible
    // default the free-form prompts lack.
    #[test]
    fn multiselect_resolves_whenever_it_cannot_ask() {
        for mut io in [piped(), io_with(false, true, false, true)] {
            let options = vec!["a".to_string(), "b".to_string()];

            let all = io
                .prompt_multiselect("Pick:", options.clone(), true)
                .expect("resolves without asking");
            let none = io
                .prompt_multiselect("Pick:", options, false)
                .expect("resolves without asking");

            assert_eq!(all, vec![0, 1]);
            assert!(none.is_empty());
        }
    }

    #[test]
    fn cleanup_bytes_cursor_only_when_no_alt_screen() {
        assert_eq!(cleanup_bytes_for(false), b"\x1b[?25h");
    }

    #[test]
    fn cleanup_bytes_exits_alt_screen_when_active() {
        assert_eq!(cleanup_bytes_for(true), b"\x1b[?1049l\x1b[?25h");
    }

    // -- partition_picked --

    fn items() -> Vec<&'static str> {
        vec!["a", "b", "c", "d"]
    }

    #[test]
    fn partition_splits_on_index_membership() {
        let (picked, declined) = partition_picked(&items(), &[0, 2]);
        assert_eq!(picked, vec!["a", "c"]);
        assert_eq!(declined, vec!["b", "d"]);
    }

    #[test]
    fn partition_preserves_order_regardless_of_pick_order() {
        let (picked, _) = partition_picked(&items(), &[3, 1]);
        assert_eq!(picked, vec!["b", "d"]);
    }

    #[test]
    fn partition_none_picked_declines_everything() {
        let (picked, declined) = partition_picked(&items(), &[]);
        assert!(picked.is_empty());
        assert_eq!(declined, items());
    }

    #[test]
    fn partition_all_picked_declines_nothing() {
        let (picked, declined) = partition_picked(&items(), &[0, 1, 2, 3]);
        assert_eq!(picked, items());
        assert!(declined.is_empty());
    }

    #[test]
    fn partition_ignores_out_of_range_indices() {
        let (picked, declined) = partition_picked(&items(), &[1, 99]);
        assert_eq!(picked, vec!["b"]);
        assert_eq!(declined, vec!["a", "c", "d"]);
    }
}
