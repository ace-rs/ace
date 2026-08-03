use std::path::{Path, PathBuf};

use super::MigrateError;
use crate::ace::Ace;
use crate::config::paths::ace_import_cache_dir;

/// Import sources used to be cached at `imports/<owner>/<repo>`, which assumed GitHub
/// and let a hostile source string escape the cache root. They are now keyed by host
/// (`imports/<host>/<path…>`). The old clones are re-fetchable, so they are deleted
/// rather than moved.
pub const VERSION: &str = "2026-07-26";

pub fn run(ace: &mut Ace) -> Result<String, MigrateError> {
    let cache_root = ace_import_cache_dir()?;
    let flat = flat_layout_entries(&cache_root);
    if flat.is_empty() {
        return Ok(String::new());
    }

    let mut removed = 0;
    for entry in &flat {
        match std::fs::remove_dir_all(entry) {
            Ok(()) => removed += 1,
            Err(e) => super::warn_left_behind(ace, entry, &format!("could not remove it ({e})")),
        }
    }

    if removed == 0 {
        return Ok(String::new());
    }

    Ok(format!(
        "host-scoped import paths; removed {removed} stale clone{} from {}",
        if removed == 1 { "" } else { "s" },
        cache_root.display(),
    ))
}

/// Old entries are owner names, new ones are hosts — and a host always carries a dot.
/// A single-label host (`localhost`) is misread as an owner and its cache re-cloned,
/// which costs a fetch and nothing else.
fn flat_layout_entries(cache_root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache_root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter(|e| !e.file_name().to_string_lossy().contains('.'))
        .map(|e| e.path())
        .collect()
}
