use crate::ace::Ace;
use crate::upgrade::{check, download, replace, target_triple};

pub fn run(ace: &mut Ace, silent: bool, force: bool, version: Option<String>) {
    let result = run_inner(ace, silent, force, version);
    if let Err(e) = result {
        if !silent {
            ace.error(&e.to_string());
        }
        std::process::exit(1);
    }
}

fn run_inner(
    ace: &mut Ace,
    silent: bool,
    force: bool,
    version: Option<String>,
) -> Result<(), super::CmdError> {
    if std::env::var("ACE_SKIP_UPDATE").as_deref() == Ok("1") {
        if !silent { ace.done("update check skipped (ACE_SKIP_UPDATE=1)"); }
        return Ok(());
    }
    if let Ok(r) = ace.require_resolved() && r.skip_update.value {
        if !silent { ace.done("update check skipped (skip_update = true)"); }
        return Ok(());
    }

    let current = semver::Version::parse(env!("CARGO_PKG_VERSION"))
        .expect("CARGO_PKG_VERSION is valid semver");
    let target_version = resolve_target_version(ace, silent, force, version.as_deref())?;

    if !force && !check::needs_update(&current, &target_version) {
        if !silent { ace.done(&format!("already at latest version ({current})")); }
        return Ok(());
    }

    let url = download::build_download_url(&target_version, target_triple());
    if !silent { ace.progress(&format!("downloading ace {target_version}...")); }

    let binary = ureq::get(&url)
        .call()
        .map_err(|e| super::CmdError::Other(format!("download failed: {e}")))?
        .body_mut()
        .read_to_vec()
        .map_err(|e| super::CmdError::Other(format!("download read failed: {e}")))?;

    let exe_path = std::env::current_exe()
        .map_err(|e| super::CmdError::Other(format!("cannot locate binary: {e}")))?;

    if replace::is_homebrew_managed(&exe_path) {
        return Err(super::CmdError::Other(
            "this binary is managed by Homebrew — run `brew upgrade ace` instead".to_string(),
        ));
    }

    replace::replace_binary(&exe_path, &binary)?;

    if let Some(marker) = check::cache_marker_path() {
        let _ = check::write_cache_marker(&marker, &target_version);
    }

    if !silent { ace.done(&format!("upgraded to {target_version}")); }
    Ok(())
}

fn resolve_target_version(
    ace: &mut Ace,
    silent: bool,
    force: bool,
    version: Option<&str>,
) -> Result<semver::Version, super::CmdError> {
    if let Some(v) = version {
        if !force {
            return Err(super::CmdError::Other("specific version requires --force".to_string()));
        }
        return semver::Version::parse(v)
            .map_err(|e| super::CmdError::Other(format!("invalid version: {e}")));
    }

    if !silent { ace.progress("checking for updates..."); }

    let latest = check::fetch_latest_version()
        .map_err(super::CmdError::Other)?;

    if let Some(marker) = check::cache_marker_path() {
        let _ = check::write_cache_marker(&marker, &latest);
    }

    Ok(latest)
}
