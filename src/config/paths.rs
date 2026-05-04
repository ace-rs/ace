use std::path::PathBuf;

use super::ConfigError;
use crate::paths;

pub struct AcePaths {
    pub user: PathBuf,
    pub project: PathBuf,
    pub local: PathBuf,
    pub cache: PathBuf,
}

pub fn resolve(project_dir: &std::path::Path) -> Result<AcePaths, ConfigError> {
    let user = paths::user_config_dir().ok_or(ConfigError::NoConfigDir)?.join("ace/ace.toml");
    let project = project_dir.join("ace.toml");
    let local = project_dir.join("ace.local.toml");
    let cache = ace_cache_dir()?;

    Ok(AcePaths { user, project, local, cache })
}

/// ACE's cache root: `<user_cache_dir>/ace`.
pub fn ace_cache_dir() -> Result<PathBuf, ConfigError> {
    paths::user_cache_dir()
        .ok_or(ConfigError::NoCacheDir)
        .map(|d| d.join("ace"))
}

/// ACE's data root: `<user_data_dir>/ace`. Holds school clones (schools are user data,
/// not cache — `UpdateOutcome::Dirty`/`AheadOfOrigin` can carry in-progress work).
pub fn ace_data_dir() -> Result<PathBuf, ConfigError> {
    paths::user_data_dir()
        .ok_or(ConfigError::NoDataDir)
        .map(|d| d.join("ace"))
}

/// Cache root for imported upstream source repos: `<user_cache_dir>/ace/imports`.
/// Read-only snapshots — safe to sweep.
pub fn ace_import_cache_dir() -> Result<PathBuf, ConfigError> {
    ace_cache_dir().map(|c| c.join("imports"))
}

/// Detect stray directories under the old cache layout — any top-level directory
/// other than `imports/` signals a pre-PROD9-76 install (or a legacy `index.toml`
/// file left over from before the index moved to the data dir). Non-directory
/// files that aren't `index.toml` (e.g. the self-update `latest_version` cache)
/// are left alone. Returns the list of stray entry paths so the caller can print
/// a one-time hint.
pub fn detect_stray_cache_dirs(cache_root: &std::path::Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(cache_root) else {
        return Vec::new();
    };

    entries
        .filter_map(Result::ok)
        .filter(|e| {
            let name = e.file_name();
            if name == "imports" {
                return false;
            }
            if name == "index.toml" {
                return true;
            }
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
        })
        .map(|e| e.path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_stray_cache_dirs_flags_owner_repo_dirs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cache_root = tmp.path();

        std::fs::create_dir_all(cache_root.join("imports/owner/repo")).unwrap();
        std::fs::create_dir_all(cache_root.join("prod9/school")).unwrap();
        std::fs::create_dir_all(cache_root.join("other-owner/other-repo")).unwrap();

        let stray = detect_stray_cache_dirs(cache_root);

        assert!(
            stray.iter().any(|p| p.ends_with("prod9")),
            "should flag prod9/ as stray; got {stray:?}",
        );
        assert!(
            stray.iter().any(|p| p.ends_with("other-owner")),
            "should flag other-owner/ as stray; got {stray:?}",
        );
        assert!(
            !stray.iter().any(|p| p.ends_with("imports")),
            "imports/ is the new cache layout — should not be flagged; got {stray:?}",
        );
    }

    #[test]
    fn detect_stray_cache_dirs_flags_legacy_index_toml() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cache_root = tmp.path();

        std::fs::create_dir_all(cache_root.join("imports")).unwrap();
        std::fs::write(cache_root.join("index.toml"), "").unwrap();

        let stray = detect_stray_cache_dirs(cache_root);
        assert!(
            stray.iter().any(|p| p.ends_with("index.toml")),
            "legacy index.toml should be flagged as stray; got {stray:?}",
        );
    }

    #[test]
    fn detect_stray_cache_dirs_ignores_latest_version_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cache_root = tmp.path();

        std::fs::create_dir_all(cache_root.join("imports")).unwrap();
        std::fs::write(cache_root.join("latest_version"), "0.4.1").unwrap();

        let stray = detect_stray_cache_dirs(cache_root);
        assert!(
            !stray.iter().any(|p| p.ends_with("latest_version")),
            "latest_version (self-update cache) is not old-layout garbage; got {stray:?}",
        );
    }

    #[test]
    fn detect_stray_cache_dirs_empty_when_clean() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let cache_root = tmp.path();

        std::fs::create_dir_all(cache_root.join("imports")).unwrap();

        let stray = detect_stray_cache_dirs(cache_root);
        assert!(stray.is_empty(), "clean cache should report no stray; got {stray:?}");
    }

    #[test]
    fn detect_stray_cache_dirs_handles_missing_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let missing = tmp.path().join("does-not-exist");

        let stray = detect_stray_cache_dirs(&missing);
        assert!(stray.is_empty(), "missing cache root should report no stray");
    }
}
