use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::ConfigError;
use super::paths::{ace_cache_dir, ace_data_dir};

/// ISO date of the newest on-disk layout migration this binary knows about. Recorded in
/// `index.toml` — ACE's only internal metadata file — so an older binary can tell "state
/// from the future" apart from "state I have never seen". See `docs/spec/migrations.md`.
pub const LAYOUT_VERSION: &str = "2026-07-26";

/// ~/.local/share/ace/index.toml — tracks downloaded schools. Schools themselves live
/// alongside at `~/.local/share/ace/{owner}/{repo}/` — index is user data, not cache,
/// so losing it to cache sweep would silently forget installed schools.
///
/// ```toml
/// layout_version = "2026-07-26"
///
/// [[school]]
/// specifier = "ace-rs/school"
/// repo = "ace-rs/school"
/// path = ""
///
/// [[school]]
/// specifier = "sith/holocron:school"
/// repo = "sith/holocron"
/// path = "school"
/// ```
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct IndexToml {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub layout_version: String,
    pub school: Vec<SchoolEntry>,
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct SchoolEntry {
    pub specifier: String,
    pub repo: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub path: String,
}

pub fn index_path() -> Result<PathBuf, ConfigError> {
    Ok(ace_data_dir()?.join("index.toml"))
}

pub fn legacy_index_path() -> Result<PathBuf, ConfigError> {
    Ok(ace_cache_dir()?.join("index.toml"))
}

pub fn load(path: &Path) -> Result<IndexToml, ConfigError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(IndexToml::default()),
        Err(e) => return Err(e.into()),
    };
    let index: IndexToml = toml::from_str(&content)?;
    Ok(index)
}

/// Write the index, stamping the current layout. Only a binary that has already
/// migrated ever writes this file, so what it writes is current by definition —
/// which is also how a fresh install ends up versioned without a migration running.
pub fn save(path: &Path, index: &IndexToml) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let stamped = IndexToml {
        layout_version: LAYOUT_VERSION.to_string(),
        school: index.school.clone(),
    };

    let content = toml::to_string(&stamped)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Parse a specifier into (repo, path) components.
fn split_specifier(specifier: &str) -> (&str, &str) {
    match specifier.split_once(':') {
        Some((repo, path)) => {
            let path = path.trim_start_matches('/');
            (repo, path)
        }
        None => (specifier, ""),
    }
}

/// Add or update a school entry in the index. Deduplicates by specifier.
pub fn upsert(index: &mut IndexToml, specifier: &str) {
    let (repo, path) = split_specifier(specifier);
    let entry = SchoolEntry {
        specifier: specifier.to_string(),
        repo: repo.to_string(),
        path: path.to_string(),
    };

    if let Some(existing) = index.school.iter_mut().find(|s| s.specifier == specifier) {
        *existing = entry;
    } else {
        index.school.push(entry);
    }
}

/// List all cached school specifiers.
pub fn list_specifiers(index: &IndexToml) -> Vec<String> {
    index.school.iter().map(|s| s.specifier.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_version_survives_a_roundtrip_alongside_schools() {
        // TOML rejects a bare key after a table, so the version must serialize
        // ahead of the [[school]] entries or saving an indexed install fails.
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("index.toml");

        let mut index = IndexToml::default();
        upsert(&mut index, "ace-rs/school");
        index.layout_version = "2026-07-26".to_string();

        save(&path, &index).expect("save with schools and a version");
        let loaded = load(&path).expect("load");

        assert_eq!(loaded.layout_version, "2026-07-26");
        assert_eq!(loaded.school.len(), 1);
    }

    #[test]
    fn absent_layout_version_reads_as_pre_versioning() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("index.toml");
        std::fs::write(&path, "[[school]]\nspecifier = \"ace-rs/school\"\n").expect("seed");

        let loaded = load(&path).expect("load");

        assert_eq!(loaded.layout_version, "");
        assert!(loaded.layout_version.as_str() < LAYOUT_VERSION);
    }

    #[test]
    fn unversioned_index_omits_the_key() {
        let mut index = IndexToml::default();
        upsert(&mut index, "ace-rs/school");

        let text = toml::to_string(&index).expect("serialize");

        assert!(!text.contains("layout_version"), "got: {text}");
    }

    #[test]
    fn upsert_deduplicates() {
        let mut index = IndexToml::default();
        upsert(&mut index, "ace-rs/school");
        upsert(&mut index, "ace-rs/school");
        assert_eq!(index.school.len(), 1);
    }

    #[test]
    fn upsert_multiple_schools() {
        let mut index = IndexToml::default();
        upsert(&mut index, "ace-rs/school");
        upsert(&mut index, "acme/school");
        assert_eq!(index.school.len(), 2);
    }

    #[test]
    fn load_missing_file_returns_default() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("missing").join("index.toml");
        let index = load(&path).expect("missing file should return default");
        assert!(index.school.is_empty());
    }

    #[test]
    fn roundtrip_save_load() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let path = tmp.path().join("index.toml");

        let mut index = IndexToml::default();
        upsert(&mut index, "ace-rs/school");
        upsert(&mut index, "jedi/temple:school");

        save(&path, &index).expect("save should succeed");
        let loaded = load(&path).expect("load should succeed");

        assert_eq!(loaded.school.len(), 2);
        assert_eq!(loaded.school[0].specifier, "ace-rs/school");
        assert_eq!(loaded.school[1].specifier, "jedi/temple:school");
        assert_eq!(loaded.school[1].repo, "jedi/temple");
        assert_eq!(loaded.school[1].path, "school");
    }
}
