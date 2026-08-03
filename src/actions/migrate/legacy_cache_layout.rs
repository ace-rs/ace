use std::path::{Path, PathBuf};

use super::MigrateError;
use crate::ace::Ace;
use crate::config::index_toml;
use crate::config::paths::{ace_cache_dir, detect_stray_cache_dirs};
use crate::git::Git;

/// Schools and the index moved out of the cache dir in PROD9-76 and again in
/// 2026-04-22. Both moves left the originals behind and warned about them on every
/// startup thereafter; this step finishes the job.
pub const VERSION: &str = "2026-04-22";

pub fn run(ace: &mut Ace) -> Result<String, MigrateError> {
    let cache_root = ace_cache_dir()?;
    let strays = detect_stray_cache_dirs(&cache_root);
    if strays.is_empty() {
        return Ok(String::new());
    }

    let index_moved = adopt_legacy_index(ace)?;
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
        parts.push(format!("kept {kept}, warned about above"));
    }

    Ok(parts.join("; "))
}

/// Read the pre-move `index.toml` into the data dir if that is still the only copy.
/// The legacy file itself is deleted by the stray sweep below.
///
/// Adoption is best-effort: everything in the file is re-derivable, so a legacy copy
/// we cannot read or write is warned about and abandoned rather than blocking the
/// sweep behind it. Not knowing *where* the file belongs is a different failure and
/// still propagates.
fn adopt_legacy_index(ace: &mut Ace) -> Result<bool, MigrateError> {
    let new = index_toml::index_path()?;
    let legacy = index_toml::legacy_index_path()?;

    if new.exists() || !legacy.exists() {
        return Ok(false);
    }

    match index_toml::load(&legacy).and_then(|i| index_toml::save(&new, &i)) {
        Ok(()) => Ok(true),
        Err(e) => {
            ace.warn(&format!(
                "could not adopt {} ({e}); starting a fresh index",
                legacy.display(),
            ));
            Ok(false)
        }
    }
}

/// Returns (removed, kept). A legacy clone is re-cloneable, so it goes — unless it
/// carries work that exists nowhere else, which is the one thing tear-and-rebuild is
/// not allowed to destroy.
fn remove_stale_clones(ace: &mut Ace, strays: &[PathBuf]) -> (usize, usize) {
    let mut removed = 0;
    let mut kept = 0;

    for stray in strays {
        if let Some(unsaved) = holds_unsaved_work(stray) {
            super::warn_left_behind(ace, stray, &unsaved);
            kept += 1;
            continue;
        }

        match remove(stray) {
            Ok(()) => removed += 1,
            Err(e) => {
                super::warn_left_behind(ace, stray, &format!("could not remove it ({e})"));
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
        let git = Git::new(&clone, false);

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
