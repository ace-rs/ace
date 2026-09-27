use std::path::PathBuf;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Installation {
    Installed,
    Missing,
    UnrecognizedSource,
}

#[derive(Debug)]
pub(super) struct Package {
    pub source: String,
    pub installed_path: Option<PathBuf>,
}

pub(super) fn inventory(output: &str) -> Result<Vec<Package>, String> {
    if output.trim() == "No packages installed." {
        return Ok(Vec::new());
    }

    let mut packages: Vec<Package> = Vec::new();
    let mut has_scope = false;
    let mut pending_path = false;
    for line in output.lines() {
        if line.is_empty() {
            continue;
        }
        if line == "Project packages:" {
            return Err(
                "unexpected project packages in user-only `pi list --no-approve` output".into(),
            );
        }
        if line == "User packages:" {
            has_scope = true;
            pending_path = false;
            continue;
        }
        if let Some(path) = line.strip_prefix("    ") {
            let path = PathBuf::from(path);
            if !pending_path || !path.is_absolute() {
                return Err("unrecognized installed package path in `pi list` output".into());
            }
            let package = packages
                .last_mut()
                .expect("pending path requires a package");
            package.installed_path = Some(path);
            pending_path = false;
            continue;
        }
        let source = line.strip_prefix("  ").filter(|source| !source.is_empty());
        let Some(source) = source.filter(|_| has_scope) else {
            return Err("unrecognized `pi list` output (expected Pi 0.87.1 format)".into());
        };
        let source = source.strip_suffix(" (filtered)").unwrap_or(source);
        packages.push(Package {
            source: source.into(),
            installed_path: None,
        });
        pending_path = true;
    }

    if packages.is_empty() {
        return Err("empty or unrecognized `pi list` output".into());
    }
    Ok(packages)
}

pub(super) fn installation(packages: &[Package]) -> Installation {
    let adapter = packages
        .iter()
        .filter(|package| is_adapter(&package.source));
    if adapter
        .clone()
        .any(|package| package.installed_path.is_some())
    {
        return Installation::Installed;
    }
    if packages
        .iter()
        .any(|package| !package.source.starts_with("npm:"))
    {
        return Installation::UnrecognizedSource;
    }
    Installation::Missing
}

pub(super) fn is_adapter(source: &str) -> bool {
    source == "npm:pi-mcp-adapter"
        || source
            .strip_prefix("npm:pi-mcp-adapter@")
            .is_some_and(|version| !version.is_empty())
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum ManifestError {
    Invalid(String),
    Unsupported(String),
}

pub(super) fn validate_manifest(body: &str) -> Result<(), ManifestError> {
    #[derive(serde::Deserialize)]
    struct Manifest {
        name: String,
        version: String,
    }
    let manifest: Manifest =
        serde_json::from_str(body).map_err(|error| ManifestError::Invalid(error.to_string()))?;
    if manifest.name != "pi-mcp-adapter" {
        return Err(ManifestError::Invalid(format!(
            "expected pi-mcp-adapter, found {}",
            manifest.name
        )));
    }
    let version = semver::Version::parse(&manifest.version)
        .map_err(|error| ManifestError::Invalid(error.to_string()))?;
    if version.major != 3 || !version.pre.is_empty() {
        return Err(ManifestError::Unsupported(manifest.version));
    }
    Ok(())
}

#[cfg(test)]
fn parse(output: &str) -> Result<Installation, String> {
    inventory(output).map(|packages| installation(&packages))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_an_adapter_v3_manifest_for_the_v3_config_contract() {
        assert!(validate_manifest(r#"{"name":"pi-mcp-adapter","version":"3.0.0"}"#).is_ok());
        for version in ["2.9.0", "4.0.0", "3.0.0-beta.1"] {
            let manifest = format!(r#"{{"name":"pi-mcp-adapter","version":"{version}"}}"#);
            assert!(
                matches!(
                    validate_manifest(&manifest),
                    Err(ManifestError::Unsupported(_))
                ),
                "{version}"
            );
        }
        for manifest in [
            "not json",
            "{}",
            r#"{"name":"other","version":"3.0.0"}"#,
            r#"{"name":"pi-mcp-adapter","version":"garbage"}"#,
        ] {
            assert!(
                matches!(validate_manifest(manifest), Err(ManifestError::Invalid(_))),
                "{manifest}"
            );
        }
    }

    // Pi 0.87.1's public `list` output distinguishes a declaration from an
    // existing installation by printing an indented path on the following line.
    #[test]
    fn requires_adapter_declaration_and_its_own_installed_path() {
        assert_eq!(
            parse(
                "User packages:\n  npm:pi-mcp-adapter@3.0.0\n    /agent/npm/node_modules/pi-mcp-adapter\n"
            ),
            Ok(Installation::Installed)
        );
        assert_eq!(
            parse(
                "User packages:\n  npm:pi-mcp-adapter\n  npm:other\n    /agent/npm/node_modules/other\n"
            ),
            Ok(Installation::Missing)
        );
    }

    #[test]
    fn rejects_project_inventory_from_the_user_only_probe() {
        assert!(
            parse(
                "User packages:\n  npm:other\n    /other\n\nProject packages:\n  npm:pi-mcp-adapter (filtered)\n    /project/.pi/npm/node_modules/pi-mcp-adapter\n"
            ).is_err()
        );
    }

    #[test]
    fn rejects_format_drift_instead_of_reporting_absence() {
        for output in [
            "",
            "[]",
            "Installed packages:\n",
            "User packages:\n  npm:pi-mcp-adapter\n    \n",
            "User packages:\n    /orphan",
        ] {
            assert!(parse(output).is_err(), "{output:?}");
        }
    }

    #[test]
    fn does_not_confuse_similar_package_names_or_alternative_sources() {
        assert_eq!(parse("No packages installed.\n"), Ok(Installation::Missing));
        assert_eq!(
            parse("User packages:\n  npm:pi-mcp-adapter-fake\n    /fake\n"),
            Ok(Installation::Missing)
        );
        assert_eq!(
            parse("User packages:\n  git:github.com/nicobailon/pi-mcp-adapter\n    /adapter\n"),
            Ok(Installation::UnrecognizedSource)
        );
        assert_eq!(
            parse("User packages:\n  ../my-extension\n    /adapter\n"),
            Ok(Installation::UnrecognizedSource)
        );
        assert_eq!(
            parse("User packages:\n  npm:pi-mcp-adapter\n  ../my-extension\n    /adapter\n"),
            Ok(Installation::UnrecognizedSource)
        );
    }
}
