pub mod check;
pub mod download;
pub mod replace;

use std::time::SystemTime;

use crate::ace::Ace;

#[cfg(windows)]
pub fn cleanup_old_binary(ace: &mut Ace) {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let old = replace::old_path(&exe);
    if !old.exists() {
        return;
    }
    if let Err(e) = std::fs::remove_file(&old) {
        ace.warn(&format!("failed to remove {}: {e}", old.display()));
    }
}

pub fn check_for_update(ace: &mut Ace) {
    if std::env::var("ACE_SKIP_UPDATE").as_deref() == Ok("1") {
        return;
    }
    if let Ok(r) = ace.require_resolved()
        && r.skip_update.value
    {
        return;
    }

    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("CARGO_PKG_VERSION is valid semver");

    let Some(marker_path) = check::cache_marker_path() else {
        return;
    };

    let latest = if check::is_cache_fresh(&marker_path, SystemTime::now()) {
        let Some(v) = check::read_cache_marker(&marker_path) else {
            return;
        };
        v
    } else {
        let tag_filter = format!("v{}.*", current.major);
        let Ok(tags) =
            crate::git::ls_remote_tags("https://github.com/ace-rs/ace.git", &tag_filter)
        else {
            return;
        };
        let versions = check::parse_version_tags(&tags);
        let Some(v) = check::latest_version(&versions) else {
            return;
        };
        let _ = check::write_cache_marker(&marker_path, v);
        v.clone()
    };

    if !check::needs_update(&current, &latest) {
        return;
    }

    // Homebrew-managed binaries can't self-update — hint the user instead.
    if let Ok(exe) = std::env::current_exe() {
        if replace::is_homebrew_managed(&exe) {
            ace.hint(&format!("ace {latest} available — run `brew upgrade ace`"));
            return;
        }
    }

    ace.hint(&format!("ace {latest} available — upgrading in background"));
    spawn_background_upgrade();
}

fn spawn_background_upgrade() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .args(["upgrade", "--silent"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

pub fn target_triple() -> &'static str {
    #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
    { "x86_64-unknown-linux-gnu" }
    #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
    { "aarch64-unknown-linux-gnu" }
    #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
    { "x86_64-apple-darwin" }
    #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
    { "aarch64-apple-darwin" }
    #[cfg(all(target_arch = "x86_64", target_os = "windows"))]
    { "x86_64-pc-windows-gnu" }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_triple_returns_known_platform() {
        let triple = target_triple();
        assert!(
            [
                "x86_64-unknown-linux-gnu",
                "aarch64-unknown-linux-gnu",
                "x86_64-apple-darwin",
                "aarch64-apple-darwin",
                "x86_64-pc-windows-gnu",
            ]
            .contains(&triple),
            "unexpected triple: {triple}"
        );
    }
}
