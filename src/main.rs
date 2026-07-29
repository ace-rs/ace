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
mod skills;
mod templates;
mod upgrade;

use ace::Io;
use clap::Parser;
use cmd::Cli;

fn main() {
    let cli = Cli::parse();
    let io = Io::new(cli.porcelain, cli.yes);

    let logo = io.logo();
    if !logo.is_empty() {
        eprintln!("{logo}");
        eprintln!("  {}\n", env!("ACE_GIT_HASH"));
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
