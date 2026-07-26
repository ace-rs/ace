use std::path::{Path, PathBuf};

use super::MigrateError;
use crate::ace::{Ace, OutputMode};
use crate::config::index_toml;
use crate::config::paths::{ace_cache_dir, detect_stray_cache_dirs};
use crate::git::Git;

/// Schools and the index moved out of the cache dir in PROD9-76 and again in
/// 2026-04-22. Both moves left the originals behind and warned about them on every
/// startup thereafter; this step finishes the job.
pub const VERSION: &str = "2026-04-22";

pub fn run(ace: &mut Ace) -> Result<Option<String>, MigrateError> {
    let cache_root = ace_cache_dir()?;
    let strays = detect_stray_cache_dirs(&cache_root);
    if strays.is_empty() {
        return Ok(None);
    }

    let index_moved = adopt_legacy_index()?;
    let (removed, kept) = remove_stale_clones(ace, &strays);

    let mut parts = Vec::new();
    if index_moved {
        parts.push(format!("adopted index.toml from {}", cache_root.display()));
    }
    if removed > 0 {
        parts.push(format!(
            "removed {removed} stale entr{} from {}",
            if removed == 1 { "y" } else { "ies" },
            cache_root.display(),
        ));
    }
    if kept > 0 {
        parts.push(format!("kept {kept} with local work"));
    }

    match parts.is_empty() {
        true => Ok(None),
        false => Ok(Some(parts.join("; "))),
    }
}

/// Read the pre-move `index.toml` into the data dir if that is still the only copy.
/// The legacy file itself is deleted by the stray sweep below.
fn adopt_legacy_index() -> Result<bool, MigrateError> {
    let new = index_toml::index_path()?;
    let legacy = index_toml::legacy_index_path()?;

    if new.exists() || !legacy.exists() {
        return Ok(false);
    }

    let index = index_toml::load(&legacy)?;
    index_toml::save(&new, &index)?;
    Ok(true)
}

/// Returns (removed, kept). A legacy clone is re-cloneable, so it goes — unless it
/// carries work that exists nowhere else, which is the one thing tear-and-rebuild is
/// not allowed to destroy.
fn remove_stale_clones(ace: &mut Ace, strays: &[PathBuf]) -> (usize, usize) {
    let mut removed = 0;
    let mut kept = 0;

    for stray in strays {
        if let Some(unsaved) = holds_unsaved_work(stray) {
            ace.warn(&format!(
                "keeping {} — {unsaved} (delete it once the work is pushed)",
                stray.display(),
            ));
            kept += 1;
            continue;
        }

        match remove(stray) {
            Ok(()) => removed += 1,
            Err(e) => {
                ace.warn(&format!("could not remove {}: {e}", stray.display()));
                kept += 1;
            }
        }
    }

    (removed, kept)
}

fn remove(path: &Path) -> std::io::Result<()> {
    match path.is_dir() {
        true => std::fs::remove_dir_all(path),
        false => std::fs::remove_file(path),
    }
}

/// Describe why a legacy tree must be kept, or `None` when it is safe to delete.
/// A repo we cannot interrogate counts as unsaved: the cost of guessing wrong is
/// destroyed work, so the check fails closed.
fn holds_unsaved_work(path: &Path) -> Option<String> {
    let clones = clones_within(path);

    for clone in clones {
        let git = Git::new(&clone, OutputMode::Silent);

        let dirty = git.is_dirty().unwrap_or(true);
        if dirty {
            return Some(format!("{} has uncommitted changes", clone.display()));
        }

        let Ok(branch) = git.current_branch() else {
            return Some(format!("{} is in an unreadable state", clone.display()));
        };
        if git.is_ahead_of(&format!("origin/{branch}")).unwrap_or(true) {
            return Some(format!("{} has unpushed commits", clone.display()));
        }
    }

    None
}

/// Legacy school clones sat at `<cache>/ace/<owner>/<repo>`, so a stray entry is
/// either a clone itself or one level of owner directory above them.
fn clones_within(path: &Path) -> Vec<PathBuf> {
    if path.join(".git").exists() {
        return vec![path.to_path_buf()];
    }

    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.join(".git").exists())
        .collect()
}
