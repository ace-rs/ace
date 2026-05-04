pub fn build_download_url(version: &semver::Version, target: &str) -> String {
    let suffix = if target.contains("windows") { ".exe" } else { "" };
    format!(
        "https://github.com/ace-rs/ace/releases/download/v{version}/ace-{target}{suffix}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_url_includes_version_and_target() {
        let version = semver::Version::new(0, 4, 0);
        let url = build_download_url(&version, "aarch64-apple-darwin");
        assert_eq!(
            url,
            "https://github.com/ace-rs/ace/releases/download/v0.4.0/ace-aarch64-apple-darwin"
        );
    }

    #[test]
    fn download_url_adds_v_prefix() {
        let version = semver::Version::new(1, 0, 0);
        let url = build_download_url(&version, "x86_64-unknown-linux-gnu");
        assert!(url.contains("/v1.0.0/"), "URL should contain v-prefixed version: {url}");
    }

    #[test]
    fn download_url_appends_exe_for_windows() {
        let version = semver::Version::new(0, 4, 0);
        let url = build_download_url(&version, "x86_64-pc-windows-gnu");
        assert_eq!(
            url,
            "https://github.com/ace-rs/ace/releases/download/v0.4.0/ace-x86_64-pc-windows-gnu.exe"
        );
    }
}
