//! Platform path resolution.
//!
//! Replaces the archived `dirs` crate. Uses environment variables on both
//! Unix and Windows; no FFI is needed.
//!
//! Unix: `HOME`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME` (empty == unset per XDG).
//! Windows: `USERPROFILE`, `APPDATA`, `LOCALAPPDATA`.

use std::ffi::OsString;
use std::path::PathBuf;

/// User home directory.
pub fn home_dir() -> Option<PathBuf> {
    home_dir_from(|k| std::env::var_os(k))
}

/// Per-user config base directory (e.g. `~/.config` on Unix, `%APPDATA%` on Windows).
pub fn user_config_dir() -> Option<PathBuf> {
    user_config_dir_from(|k| std::env::var_os(k))
}

/// Per-user cache base directory (e.g. `~/.cache` on Unix, `%LOCALAPPDATA%` on Windows).
pub fn user_cache_dir() -> Option<PathBuf> {
    user_cache_dir_from(|k| std::env::var_os(k))
}

/// Per-user data base directory (e.g. `~/.local/share` on Unix, `%APPDATA%` on Windows).
pub fn user_data_dir() -> Option<PathBuf> {
    user_data_dir_from(|k| std::env::var_os(k))
}

// -- inner fns (env injected for tests) --

#[cfg(unix)]
fn home_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("HOME")).map(PathBuf::from)
}

#[cfg(unix)]
fn user_config_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("XDG_CONFIG_HOME"))
        .map(PathBuf::from)
        .or_else(|| home_dir_from(&get).map(|h| h.join(".config")))
}

#[cfg(unix)]
fn user_cache_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("XDG_CACHE_HOME"))
        .map(PathBuf::from)
        .or_else(|| home_dir_from(&get).map(|h| h.join(".cache")))
}

#[cfg(unix)]
fn user_data_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("XDG_DATA_HOME"))
        .map(PathBuf::from)
        .or_else(|| home_dir_from(&get).map(|h| h.join(".local/share")))
}

#[cfg(windows)]
fn home_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("USERPROFILE")).map(PathBuf::from)
}

#[cfg(windows)]
fn user_config_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("APPDATA")).map(PathBuf::from)
}

#[cfg(windows)]
fn user_cache_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("LOCALAPPDATA")).map(PathBuf::from)
}

#[cfg(windows)]
fn user_data_dir_from<F: Fn(&str) -> Option<OsString>>(get: F) -> Option<PathBuf> {
    non_empty(get("APPDATA")).map(PathBuf::from)
}

fn non_empty(v: Option<OsString>) -> Option<OsString> {
    v.filter(|s| !s.is_empty())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env_from(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).map(OsString::from)
    }

    // -- home_dir --

    #[test]
    fn home_dir_returns_home() {
        let get = env_from(&[("HOME", "/tmp/home-foo")]);
        assert_eq!(home_dir_from(get), Some(PathBuf::from("/tmp/home-foo")));
    }

    #[test]
    fn home_dir_unset_is_none() {
        let get = env_from(&[]);
        assert_eq!(home_dir_from(get), None);
    }

    #[test]
    fn home_dir_empty_is_none() {
        let get = env_from(&[("HOME", "")]);
        assert_eq!(home_dir_from(get), None);
    }

    // -- user_config_dir --

    #[test]
    fn config_dir_uses_xdg_when_set() {
        let get = env_from(&[
            ("XDG_CONFIG_HOME", "/tmp/xdg-foo"),
            ("HOME", "/tmp/home-foo"),
        ]);
        assert_eq!(
            user_config_dir_from(get),
            Some(PathBuf::from("/tmp/xdg-foo"))
        );
    }

    #[test]
    fn config_dir_falls_back_to_home_dot_config_when_xdg_unset() {
        let get = env_from(&[("HOME", "/tmp/home-foo")]);
        assert_eq!(
            user_config_dir_from(get),
            Some(PathBuf::from("/tmp/home-foo/.config")),
        );
    }

    #[test]
    fn config_dir_treats_empty_xdg_as_unset() {
        let get = env_from(&[("XDG_CONFIG_HOME", ""), ("HOME", "/tmp/home-foo")]);
        assert_eq!(
            user_config_dir_from(get),
            Some(PathBuf::from("/tmp/home-foo/.config")),
        );
    }

    #[test]
    fn config_dir_none_when_nothing_set() {
        let get = env_from(&[]);
        assert_eq!(user_config_dir_from(get), None);
    }

    // -- user_cache_dir --

    #[test]
    fn cache_dir_uses_xdg_when_set() {
        let get = env_from(&[
            ("XDG_CACHE_HOME", "/tmp/xdg-cache"),
            ("HOME", "/tmp/home-foo"),
        ]);
        assert_eq!(
            user_cache_dir_from(get),
            Some(PathBuf::from("/tmp/xdg-cache"))
        );
    }

    #[test]
    fn cache_dir_falls_back_to_home_dot_cache_when_xdg_unset() {
        let get = env_from(&[("HOME", "/tmp/home-foo")]);
        assert_eq!(
            user_cache_dir_from(get),
            Some(PathBuf::from("/tmp/home-foo/.cache")),
        );
    }

    #[test]
    fn cache_dir_treats_empty_xdg_as_unset() {
        let get = env_from(&[("XDG_CACHE_HOME", ""), ("HOME", "/tmp/home-foo")]);
        assert_eq!(
            user_cache_dir_from(get),
            Some(PathBuf::from("/tmp/home-foo/.cache")),
        );
    }

    #[test]
    fn cache_dir_none_when_nothing_set() {
        let get = env_from(&[]);
        assert_eq!(user_cache_dir_from(get), None);
    }

    // -- user_data_dir --

    #[test]
    fn data_dir_uses_xdg_when_set() {
        let get = env_from(&[
            ("XDG_DATA_HOME", "/tmp/xdg-data"),
            ("HOME", "/tmp/home-foo"),
        ]);
        assert_eq!(
            user_data_dir_from(get),
            Some(PathBuf::from("/tmp/xdg-data"))
        );
    }

    #[test]
    fn data_dir_falls_back_to_home_dot_local_share_when_xdg_unset() {
        let get = env_from(&[("HOME", "/tmp/home-foo")]);
        assert_eq!(
            user_data_dir_from(get),
            Some(PathBuf::from("/tmp/home-foo/.local/share")),
        );
    }

    #[test]
    fn data_dir_treats_empty_xdg_as_unset() {
        let get = env_from(&[("XDG_DATA_HOME", ""), ("HOME", "/tmp/home-foo")]);
        assert_eq!(
            user_data_dir_from(get),
            Some(PathBuf::from("/tmp/home-foo/.local/share")),
        );
    }

    #[test]
    fn data_dir_none_when_nothing_set() {
        let get = env_from(&[]);
        assert_eq!(user_data_dir_from(get), None);
    }
}
