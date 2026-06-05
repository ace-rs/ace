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

use ace::OutputMode;
use clap::Parser;
use cmd::Cli;

fn main() {
    let cli = Cli::parse();
    let mode = OutputMode::detect(cli.porcelain);

    let logo = ace::logo(mode);
    if !logo.is_empty() {
        eprintln!("{logo}");
        eprintln!("  {}\n", env!("ACE_GIT_HASH"));
    }

    let project_dir = std::env::current_dir().expect("cannot determine current directory");
    let mut ace = ace::Ace::new(project_dir, mode);
    migrate_legacy_index_toml(&mut ace);
    warn_stray_cache_dirs(&mut ace);
    cmd::run(&mut ace, cli);
}

/// One-shot read-migration for the pre-PROD9-76 `~/.cache/ace/index.toml` location.
/// Moves content to `~/.local/share/ace/index.toml` on first startup after upgrade.
/// Legacy file is left in place and surfaced by `warn_stray_cache_dirs`. Silent on
/// failure — later setup/clone paths retry, and the user's data is never lost.
fn migrate_legacy_index_toml(ace: &mut ace::Ace) {
    let (Ok(new), Ok(legacy)) = (
        config::index_toml::index_path(),
        config::index_toml::legacy_index_path(),
    ) else {
        return;
    };
    if new.exists() || !legacy.exists() {
        return;
    }
    if config::index_toml::load_or_migrate(&new, &legacy).is_ok() {
        ace.done(&format!(
            "Migrated index.toml to {} (legacy file left at {} for manual cleanup)",
            new.display(),
            legacy.display(),
        ));
    }
}

/// Startup hint: if the old flat cache layout (`~/.cache/ace/{owner/repo}/`)
/// has stray entries, nudge the user to clean them up. Self-silences once the user
/// deletes the strays. New layout: `~/.local/share/ace/` (schools) +
/// `~/.cache/ace/imports/` (upstream source snapshots).
fn warn_stray_cache_dirs(ace: &mut ace::Ace) {
    let Ok(cache_root) = config::paths::ace_cache_dir() else {
        return;
    };

    let stray = config::paths::detect_stray_cache_dirs(&cache_root);
    if stray.is_empty() {
        return;
    }

    ace.warn(&format!(
        "old ACE cache layout detected at {} ({} stray entr{}); safe to delete",
        cache_root.display(),
        stray.len(),
        if stray.len() == 1 { "y" } else { "ies" },
    ));
}
