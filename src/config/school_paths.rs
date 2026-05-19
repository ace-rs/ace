use std::path::PathBuf;

use super::ConfigError;
use super::paths::ace_data_dir;

#[derive(Clone, Debug)]
pub struct SchoolPaths {
    pub source: String,
    pub clone_path: Option<PathBuf>,
    pub root: PathBuf,
}

pub fn resolve(
    project_dir: &std::path::Path,
    specifier: &str,
) -> Result<SchoolPaths, ConfigError> {
    let (source, path) = parse_specifier(specifier)?;
    let (base, clone_path) = if source == "." {
        (project_dir.to_path_buf(), None)
    } else {
        let clone_path = ace_data_dir()?.join(&source);
        (clone_path.clone(), Some(clone_path))
    };
    let root = path.map(|p| base.join(p)).unwrap_or(base.clone());
    Ok(SchoolPaths {
        source: specifier.to_string(),
        clone_path,
        root,
    })
}

/// Parse "source:path" specifier into (source, optional path).
fn parse_specifier(spec: &str) -> Result<(String, Option<&str>), ConfigError> {
    let (source, path) = match spec.split_once(':') {
        Some((source, path)) => {
            let path = path.trim_start_matches('/');
            (source, if path.is_empty() { None } else { Some(path) })
        }
        None => (spec, None),
    };

    if source != "." && has_traversal(source) {
        return Err(ConfigError::TraversalInSource(source.to_string()));
    }

    if let Some(p) = path
        && has_traversal(p)
    {
        return Err(ConfigError::TraversalInPath(p.to_string()));
    }

    Ok((source.to_string(), path))
}

fn has_traversal(s: &str) -> bool {
    s.split('/').any(|seg| seg == "." || seg == "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_specifier_valid() {
        let cases: &[(&str, &str, Option<&str>)] = &[
            ("ace-rs/school", "ace-rs/school", None),
            ("jedi/temple:school", "jedi/temple", Some("school")),
            (".", ".", None),
            (".:/school", ".", Some("school")),
            (".:school", ".", Some("school")),
            ("sith/order:/deep/path", "sith/order", Some("deep/path")),
            ("owner/repo:", "owner/repo", None),
        ];

        for (input, expected_source, expected_path) in cases {
            let (source, path) = parse_specifier(input).unwrap_or_else(|e| {
                panic!("parse_specifier({input:?}) should succeed but got: {e}")
            });
            assert_eq!(source, *expected_source, "source mismatch for {input:?}");
            assert_eq!(path, *expected_path, "path mismatch for {input:?}");
        }
    }

    #[test]
    fn parse_specifier_rejects_traversal() {
        let cases: &[&str] = &[
            "../escape",
            "owner/../etc",
            "./sneaky",
            "owner/repo:../secret",
            "owner/repo:foo/../../etc",
            "owner/repo:./here",
            "owner/repo:foo/./bar",
            "../../../etc/passwd",
            "owner/repo:../../../etc/passwd",
        ];

        for input in cases {
            let result = parse_specifier(input);
            assert!(result.is_err(), "parse_specifier({input:?}) should fail but got: {result:?}");
        }
    }

    #[test]
    fn resolve_embedded() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("myproject");
        let cases: &[(&str, PathBuf)] = &[
            (".:/school", project.join("school")),
            (".:school", project.join("school")),
            (".", project.clone()),
        ];

        for (spec, expected_root) in cases {
            let p = resolve(&project, spec)
                .expect("resolve should succeed for embedded spec");
            assert!(p.clone_path.is_none(), "embedded school should have no clone path for {spec:?}");
            assert_eq!(&p.root, expected_root, "root mismatch for {spec:?}");
        }
    }

    #[test]
    fn resolve_remote() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("myproject");
        let data_root = super::super::paths::ace_data_dir()
            .expect("ace_data_dir should resolve in tests");
        let cases: &[(&str, &str, &str)] = &[
            ("ace-rs/school", "ace/ace-rs/school", "ace/ace-rs/school"),
            ("sith/holocron:school", "ace/sith/holocron", "ace/sith/holocron/school"),
        ];

        for (spec, clone_suffix, root_suffix) in cases {
            let p = resolve(&project, spec)
                .expect("resolve should succeed for remote spec");

            let clone = p.clone_path.as_ref()
                .expect("clone_path should be Some for remote spec");
            assert!(
                clone.starts_with(&data_root),
                "clone {clone:?} should live under data dir {data_root:?}"
            );
            assert!(clone.ends_with(clone_suffix), "clone {clone:?} should end with {clone_suffix:?}");
            assert!(p.root.ends_with(root_suffix), "root {:?} should end with {root_suffix:?}", p.root);
        }
    }

    #[test]
    fn resolve_rejects_traversal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let project = tmp.path().join("myproject");
        let result = resolve(&project, "owner/repo:../secret");
        assert!(result.is_err());
    }
}
