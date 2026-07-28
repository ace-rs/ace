use std::collections::HashSet;
use std::io::{IsTerminal, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use console::style;
use indicatif::{ProgressBar, ProgressStyle};

// ANSI escape sequences for terminal state management.
// Alt screen is a separate buffer that preserves the user's scrollback.
// Cursor hide/show prevents a flickering cursor during full-screen redraws.
const ENTER_ALT_SCREEN: &[u8] = b"\x1b[?1049h"; // switch to alt screen buffer
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

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    #[default]
    Human,
    Porcelain,
    Silent,
}

impl OutputMode {
    pub fn detect(porcelain: bool) -> Self {
        if porcelain || !std::io::stderr().is_terminal() {
            Self::Porcelain
        } else {
            Self::Human
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("cancelled")]
    Cancelled,
    #[error("no terminal to answer \"{prompt}\"")]
    NoTerminal { prompt: String },
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl IoError {
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::NoTerminal { .. } => {
                Some("run this in an interactive terminal, or set the value with `ace config set`")
            }
            Self::Cancelled | Self::Io(_) => None,
        }
    }
}

pub struct Io {
    mode: OutputMode,
    spinner: Option<ProgressBar>,
    guard: Option<TerminalGuard>,
}

#[allow(dead_code)]
pub const LOGO: &str = r"
░█▀█░█▀▀░█▀▀
░█▀█░█░░░█▀▀
░▀░▀░▀▀▀░▀▀▀";

pub const LOGO_COLOR: &str = "\x1b[36m
░█▀█░█▀▀░█▀▀
░█▀█░█░░░█▀▀
░▀░▀░▀▀▀░▀▀▀\x1b[0m";

pub fn logo(mode: OutputMode) -> &'static str {
    match mode {
        OutputMode::Human => LOGO_COLOR,
        _ => "",
    }
}

impl Io {
    pub fn new(mode: OutputMode) -> Self {
        let guard = match mode {
            OutputMode::Human => Some(TerminalGuard::new()),
            _ => None,
        };
        Self {
            mode,
            spinner: None,
            guard,
        }
    }

    /// Enter alternate screen buffer. The guard will exit it on drop/SIGINT.
    /// No-op in Porcelain/Silent mode (no terminal to manage).
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
        if self.mode != OutputMode::Human {
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
        self.clear_spinner();
        match self.mode {
            OutputMode::Human => eprintln!("{} {msg}", style("✓").green()),
            OutputMode::Porcelain => eprintln!("{msg}"),
            OutputMode::Silent => {}
        }
    }

    pub fn info(&mut self, msg: &str) {
        self.clear_spinner();
        match self.mode {
            OutputMode::Human => eprintln!("  {msg}"),
            OutputMode::Porcelain => eprintln!("{msg}"),
            OutputMode::Silent => {}
        }
    }

    pub fn warn(&mut self, msg: &str) {
        self.clear_spinner();
        match self.mode {
            OutputMode::Human => eprintln!("{} {msg}", style("⚠").yellow()),
            OutputMode::Porcelain => eprintln!("{msg}"),
            OutputMode::Silent => {}
        }
    }

    pub fn error(&mut self, msg: &str) {
        self.clear_spinner();
        match self.mode {
            OutputMode::Human => eprintln!("{} {msg}", style("✗").red()),
            OutputMode::Porcelain => eprintln!("{msg}"),
            OutputMode::Silent => {}
        }
    }

    pub fn hint(&mut self, msg: &str) {
        self.clear_spinner();
        match self.mode {
            OutputMode::Human => eprintln!("  {} {msg}", style("→").dim()),
            OutputMode::Porcelain => eprintln!("hint: {msg}"),
            OutputMode::Silent => {}
        }
    }

    pub fn data(&mut self, msg: &str) {
        self.clear_spinner();
        if self.mode == OutputMode::Silent {
            return;
        }
        println!("{msg}");
    }

    pub fn separator(&mut self) {
        self.clear_spinner();
        if self.mode == OutputMode::Human {
            eprintln!("\n{}\n", style("⟢⟢⟢⟢⟢⟢⟢").dim());
        }
    }

    // -- input --

    pub fn prompt_text(&mut self, prompt: &str, initial: Option<&str>) -> Result<String, IoError> {
        self.require_terminal(prompt)?;

        self.clear_spinner();
        let mut p = inquire::Text::new(prompt);
        if let Some(val) = initial {
            p = p.with_initial_value(val);
        }
        p.prompt().map_err(map_inquire_err)
    }

    pub fn prompt_select(&mut self, prompt: &str, options: Vec<String>) -> Result<String, IoError> {
        self.require_terminal(prompt)?;

        self.clear_spinner();
        inquire::Select::new(prompt, options)
            .prompt()
            .map_err(map_inquire_err)
    }

    /// Free-form prompts have no defensible headless answer — unlike a
    /// checklist, which falls back to all-or-none. Refuse rather than invent
    /// one on the user's behalf.
    fn require_terminal(&self, prompt: &str) -> Result<(), IoError> {
        match self.mode {
            OutputMode::Human => Ok(()),
            OutputMode::Porcelain | OutputMode::Silent => Err(IoError::NoTerminal {
                prompt: prompt.to_string(),
            }),
        }
    }

    /// Present a checklist and return the indices of the ticked options.
    ///
    /// Indices rather than values so callers match picks back against their own
    /// list without a stringly round-trip. Outside `Human` mode there is no
    /// terminal to tick boxes in, so `default_all` decides: all or none.
    pub fn prompt_multiselect(
        &mut self,
        prompt: &str,
        options: Vec<String>,
        default_all: bool,
    ) -> Result<Vec<usize>, IoError> {
        if self.mode != OutputMode::Human {
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

    // A pipe has nowhere to type an answer. Substituting a default would put
    // words in the user's mouth, so both free-form prompts refuse instead.
    const HEADLESS: [OutputMode; 2] = [OutputMode::Porcelain, OutputMode::Silent];

    #[test]
    fn text_prompt_refuses_without_a_terminal() {
        for mode in HEADLESS {
            let mut io = Io::new(mode);

            let err = io
                .prompt_text("School name:", None)
                .expect_err("no terminal to answer on");

            assert!(matches!(err, IoError::NoTerminal { .. }));
            assert!(err.to_string().contains("School name:"));
            assert!(err.hint().is_some());
        }
    }

    #[test]
    fn select_prompt_refuses_without_a_terminal() {
        for mode in HEADLESS {
            let mut io = Io::new(mode);

            let err = io
                .prompt_select("Pick a backend:", vec!["claude".to_string()])
                .expect_err("no terminal to answer on");

            assert!(matches!(err, IoError::NoTerminal { .. }));
        }
    }

    // The checklist keeps its all-or-none resolution — it has a defensible
    // default the free-form prompts lack.
    #[test]
    fn multiselect_still_resolves_without_a_terminal() {
        let mut io = Io::new(OutputMode::Porcelain);
        let options = vec!["a".to_string(), "b".to_string()];

        let all = io
            .prompt_multiselect("Pick:", options.clone(), true)
            .expect("resolves headlessly");
        let none = io
            .prompt_multiselect("Pick:", options, false)
            .expect("resolves headlessly");

        assert_eq!(all, vec![0, 1]);
        assert!(none.is_empty());
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
