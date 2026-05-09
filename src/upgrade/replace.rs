use std::path::{Path, PathBuf};

/// Homebrew-managed prefixes. If the running binary lives under one of these,
/// `ace upgrade` should refuse and tell the user to `brew upgrade` instead.
const HOMEBREW_PREFIXES: &[&str] = &[
    "/opt/homebrew/",     // macOS ARM
    "/usr/local/Cellar/", // macOS Intel (legacy)
    "/home/linuxbrew/",   // Linux Homebrew
];

/// Returns `true` if `exe_path` appears to be managed by Homebrew.
pub fn is_homebrew_managed(exe_path: &Path) -> bool {
    let Some(s) = exe_path.to_str() else {
        return false;
    };
    HOMEBREW_PREFIXES.iter().any(|prefix| s.starts_with(prefix))
}

#[cfg(unix)]
pub fn staging_path(exe: &Path) -> PathBuf {
    let mut p = exe.as_os_str().to_owned();
    p.push(".new");
    PathBuf::from(p)
}

#[cfg(windows)]
pub fn old_path(exe: &Path) -> PathBuf {
    let mut p = exe.as_os_str().to_owned();
    p.push(".old");
    PathBuf::from(p)
}

#[cfg(unix)]
pub fn replace_binary(exe_path: &Path, new_binary: &[u8]) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let staging = staging_path(exe_path);
    std::fs::write(&staging, new_binary)?;
    std::fs::set_permissions(&staging, std::fs::Permissions::from_mode(0o755))?;
    std::fs::rename(&staging, exe_path)?;
    Ok(())
}

#[cfg(windows)]
pub fn replace_binary(exe_path: &Path, new_binary: &[u8]) -> std::io::Result<()> {
    let old = old_path(exe_path);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe_path, &old)?;
    std::fs::write(exe_path, new_binary)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staging_path_appends_new() {
        let exe = PathBuf::from("/usr/local/bin/ace");
        assert_eq!(staging_path(&exe), PathBuf::from("/usr/local/bin/ace.new"));
    }

    #[cfg(windows)]
    #[test]
    fn old_path_appends_old() {
        let exe = PathBuf::from("C:\\Program Files\\ace.exe");
        assert_eq!(old_path(&exe), PathBuf::from("C:\\Program Files\\ace.exe.old"));
    }

    #[cfg(unix)]
    #[test]
    fn replace_binary_atomic_on_unix() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exe_path = dir.path().join("ace");
        std::fs::write(&exe_path, b"old binary").unwrap();

        let new_binary = b"new binary content";
        replace_binary(&exe_path, new_binary).unwrap();

        let content = std::fs::read(&exe_path).unwrap();
        assert_eq!(content, new_binary);
    }

    #[cfg(unix)]
    #[test]
    fn replace_binary_sets_executable_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().expect("tempdir");
        let exe_path = dir.path().join("ace");
        std::fs::write(&exe_path, b"old").unwrap();

        replace_binary(&exe_path, b"new").unwrap();

        let perms = std::fs::metadata(&exe_path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o755, 0o755);
    }

    #[test]
    fn homebrew_managed_detects_arm_prefix() {
        assert!(is_homebrew_managed(Path::new("/opt/homebrew/bin/ace")));
        assert!(is_homebrew_managed(Path::new("/opt/homebrew/Cellar/ace/0.6.0/bin/ace")));
    }

    #[test]
    fn homebrew_managed_detects_intel_prefix() {
        assert!(is_homebrew_managed(Path::new("/usr/local/Cellar/ace/0.6.0/bin/ace")));
    }

    #[test]
    fn homebrew_managed_detects_linux_prefix() {
        assert!(is_homebrew_managed(Path::new("/home/linuxbrew/.linuxbrew/bin/ace")));
    }

    #[test]
    fn homebrew_managed_rejects_normal_paths() {
        assert!(!is_homebrew_managed(Path::new("/usr/local/bin/ace")));
        assert!(!is_homebrew_managed(Path::new("/home/user/.local/bin/ace")));
    }

    #[cfg(unix)]
    #[test]
    fn replace_binary_cleans_up_staging_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let exe_path = dir.path().join("ace");
        std::fs::write(&exe_path, b"old").unwrap();

        replace_binary(&exe_path, b"new").unwrap();

        let staging = staging_path(&exe_path);
        assert!(!staging.exists(), "staging file should be removed after rename");
    }
}
