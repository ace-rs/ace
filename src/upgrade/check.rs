use std::path::Path;
use std::time::{Duration, SystemTime};

const CACHE_TTL: Duration = Duration::from_secs(4 * 3600);
const LATEST_URL: &str = "https://ace-rs.dev/latest";

pub fn parse_latest_marker(s: &str) -> Option<semver::Version> {
    let trimmed = s.trim();
    let stripped = trimmed.strip_prefix('v').unwrap_or(trimmed);
    semver::Version::parse(stripped).ok()
}

pub fn fetch_latest_version() -> Result<semver::Version, String> {
    let body = ureq::get(LATEST_URL)
        .call()
        .map_err(|e| format!("fetch {LATEST_URL}: {e}"))?
        .body_mut()
        .read_to_string()
        .map_err(|e| format!("read {LATEST_URL}: {e}"))?;
    parse_latest_marker(&body).ok_or_else(|| format!("invalid version marker: {body:?}"))
}

pub fn read_cache_marker(path: &Path) -> Option<semver::Version> {
    let content = std::fs::read_to_string(path).ok()?;
    semver::Version::parse(content.trim()).ok()
}

pub fn write_cache_marker(path: &Path, version: &semver::Version) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, version.to_string())
}

pub fn is_cache_fresh(path: &Path, now: SystemTime) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(elapsed) = now.duration_since(modified) else {
        return false;
    };
    elapsed < CACHE_TTL
}

pub fn needs_update(current: &semver::Version, latest: &semver::Version) -> bool {
    latest > current
}

pub fn cache_marker_path() -> Option<std::path::PathBuf> {
    crate::paths::user_cache_dir().map(|d| d.join("ace/latest_version"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- latest marker parsing --

    #[test]
    fn parse_latest_marker_with_v_prefix() {
        assert_eq!(parse_latest_marker("v0.6.0"), Some(semver::Version::new(0, 6, 0)));
    }

    #[test]
    fn parse_latest_marker_without_v_prefix() {
        assert_eq!(parse_latest_marker("0.6.0"), Some(semver::Version::new(0, 6, 0)));
    }

    #[test]
    fn parse_latest_marker_strips_whitespace() {
        assert_eq!(parse_latest_marker("  v0.6.0\n"), Some(semver::Version::new(0, 6, 0)));
    }

    #[test]
    fn parse_latest_marker_rejects_garbage() {
        assert_eq!(parse_latest_marker("not-a-version"), None);
        assert_eq!(parse_latest_marker(""), None);
        assert_eq!(parse_latest_marker("<html>404</html>"), None);
    }

    // -- cache marker --

    #[test]
    fn read_cache_marker_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        assert!(read_cache_marker(&path).is_none());
    }

    #[test]
    fn read_cache_marker_valid() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        std::fs::write(&path, "0.4.0\n").expect("write marker");
        assert_eq!(read_cache_marker(&path), Some(semver::Version::new(0, 4, 0)));
    }

    #[test]
    fn read_cache_marker_strips_whitespace() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        std::fs::write(&path, "  0.4.0  \n").expect("write marker");
        assert_eq!(read_cache_marker(&path), Some(semver::Version::new(0, 4, 0)));
    }

    #[test]
    fn read_cache_marker_invalid_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        std::fs::write(&path, "not-a-version").expect("write marker");
        assert!(read_cache_marker(&path).is_none());
    }

    #[test]
    fn write_cache_marker_creates_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        let version = semver::Version::new(0, 4, 0);
        write_cache_marker(&path, &version).expect("write marker");
        let content = std::fs::read_to_string(&path).expect("read marker");
        assert_eq!(content.trim(), "0.4.0");
    }

    #[test]
    fn write_cache_marker_creates_parent_dirs() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("sub/dir/latest_version");
        let version = semver::Version::new(0, 4, 0);
        write_cache_marker(&path, &version).expect("write marker");
        assert!(path.exists());
    }

    // -- cache freshness --

    #[test]
    fn cache_marker_stale_after_ttl() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        std::fs::write(&path, "0.4.0").expect("write marker");

        let five_hours_later = SystemTime::now() + Duration::from_secs(5 * 3600);
        assert!(!is_cache_fresh(&path, five_hours_later));
    }

    #[test]
    fn cache_marker_fresh_within_ttl() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        std::fs::write(&path, "0.4.0").expect("write marker");
        assert!(is_cache_fresh(&path, SystemTime::now()));
    }

    #[test]
    fn cache_marker_missing_not_fresh() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("latest_version");
        assert!(!is_cache_fresh(&path, SystemTime::now()));
    }

    // -- needs_update comparison --

    #[test]
    fn needs_update_when_latest_is_newer() {
        let current = semver::Version::new(0, 3, 0);
        let latest = semver::Version::new(0, 4, 0);
        assert!(needs_update(&current, &latest));
    }

    #[test]
    fn no_update_when_equal() {
        let current = semver::Version::new(0, 3, 0);
        let latest = semver::Version::new(0, 3, 0);
        assert!(!needs_update(&current, &latest));
    }

    #[test]
    fn no_update_when_current_is_newer() {
        let current = semver::Version::new(0, 5, 0);
        let latest = semver::Version::new(0, 4, 0);
        assert!(!needs_update(&current, &latest));
    }

    // -- cache_marker_path --

    #[test]
    fn cache_marker_path_returns_some() {
        assert!(cache_marker_path().is_some());
    }
}
