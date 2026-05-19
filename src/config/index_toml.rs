use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::ConfigError;
use super::paths::{ace_cache_dir, ace_data_dir};

/// ~/.local/share/ace/index.toml — tracks downloaded schools. Schools themselves live
/// alongside at `~/.local/share/ace/{owner}/{repo}/` — index is user data, not cache,
/// so losing it to cache sweep would silently forget installed schools.
///
/// ```toml
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

/// Load the index, one-shot migrating from the legacy cache location if needed.
/// If the new path exists, use it. Otherwise if the legacy path exists, load it
/// and write it to the new path (legacy file is left in place; the startup
/// `warn_stray_cache_dirs` hint surfaces it for manual cleanup).
pub fn load_or_migrate(new: &Path, legacy: &Path) -> Result<IndexToml, ConfigError> {
    if new.exists() {
        return load(new);
    }
    if legacy.exists() {
        let index = load(legacy)?;
        save(new, &index)?;
        return Ok(index);
    }
    Ok(IndexToml::default())
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

pub fn save(path: &Path, index: &IndexToml) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = toml::to_string(index)?;
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
    fn load_or_migrate_moves_legacy_to_new() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let legacy = tmp.path().join("cache").join("index.toml");
        let new = tmp.path().join("data").join("index.toml");

        let mut seed = IndexToml::default();
        upsert(&mut seed, "ace-rs/school");
        save(&legacy, &seed).expect("seed legacy");

        let loaded = load_or_migrate(&new, &legacy).expect("migrate");
        assert_eq!(loaded.school.len(), 1);
        assert_eq!(loaded.school[0].specifier, "ace-rs/school");
        assert!(new.exists(), "new path should be written after migration");
    }

    #[test]
    fn load_or_migrate_prefers_new_when_both_exist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let legacy = tmp.path().join("cache").join("index.toml");
        let new = tmp.path().join("data").join("index.toml");

        let mut legacy_seed = IndexToml::default();
        upsert(&mut legacy_seed, "legacy/school");
        save(&legacy, &legacy_seed).expect("seed legacy");

        let mut new_seed = IndexToml::default();
        upsert(&mut new_seed, "new/school");
        save(&new, &new_seed).expect("seed new");

        let loaded = load_or_migrate(&new, &legacy).expect("load");
        assert_eq!(loaded.school.len(), 1);
        assert_eq!(loaded.school[0].specifier, "new/school");
    }

    #[test]
    fn load_or_migrate_returns_default_when_neither_exists() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let legacy = tmp.path().join("cache").join("index.toml");
        let new = tmp.path().join("data").join("index.toml");

        let loaded = load_or_migrate(&new, &legacy).expect("load");
        assert!(loaded.school.is_empty());
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
