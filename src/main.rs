#![deny(warnings)]

mod ace;
mod actions;
mod backend;
mod cmd;
mod config;
mod fsutil;
mod git;
mod glob;
mod paths;
mod platform;
mod school;
mod session;
mod skills;
mod templates;
mod upgrade;

// In-process integration tests exercise cache lifetime without exposing a public library.
#[cfg(test)]
#[path = "../tests/config_actions/mod.rs"]
mod config_action_tests;

use ace::{Io, Wordmark};
use clap::Parser;
use cmd::Cli;

fn main() {
    let cli = Cli::parse();
    let mut io = Io::new(cli.porcelain, cli.yes);

    let wordmark = io.wordmark(cli.wordmark());
    if !wordmark.is_empty() {
        match cli.wordmark() {
            Wordmark::Compact => {
                eprintln!("{wordmark} \x1b[2;90m{}\x1b[0m", cmd::BUILD_IDENTITY);
            }
            Wordmark::Big => {
                eprintln!("{wordmark}");
                io.info(&format!("version {}", cmd::BUILD_IDENTITY));
            }
            Wordmark::None => {}
        }
    }

    let project_dir = std::env::current_dir().expect("cannot determine current directory");
    let paths = match config::paths::resolve(&project_dir) {
        Ok(paths) => paths,
        // Before `Ace` exists there is no `Io` to route through, but the exit
        // class still comes from the one classifier.
        Err(e) => {
            let err = cmd::CmdError::Config(e);
            eprintln!("ace: {err}");
            std::process::exit(err.exit_code().code());
        }
    };

    let mut ace = ace::Ace::new(project_dir, paths, io);
    migrate_layout(&mut ace);
    cmd::run(&mut ace, cli);
}

/// Bring on-disk state up to the layout this binary understands, before any command
/// reads it. See `docs/spec/migrations.md`. A failure here means ACE cannot trust what
/// it is about to read, so it stops rather than operating on a half-known layout.
fn migrate_layout(ace: &mut ace::Ace) {
    let result = actions::migrate::Migrate.run(ace).map_err(Into::into);
    cmd::exit_on_err(ace, result);
}
