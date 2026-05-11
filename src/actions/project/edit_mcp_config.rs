//! Edit `exclude_mcp` in `ace.local.toml`. Tiny helpers — single use case
//! (append-dedup + idempotent remove), so no `Op` enum like `edit_skills_config`.

use std::path::Path;

use crate::config::ace_toml;
use crate::config::ConfigError;

/// Append `name` to `exclude_mcp` if not already present. Idempotent.
pub fn exclude(local_toml_path: &Path, name: &str) -> Result<(), ConfigError> {
    if let Some(parent) = local_toml_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut toml = ace_toml::load_or_default(local_toml_path)?;
    if !toml.exclude_mcp.iter().any(|n| n == name) {
        toml.exclude_mcp.push(name.to_string());
    }
    ace_toml::save(local_toml_path, &toml)
}

/// Remove `name` from `exclude_mcp`. Idempotent — no-op if absent.
pub fn include(local_toml_path: &Path, name: &str) -> Result<(), ConfigError> {
    if let Some(parent) = local_toml_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut toml = ace_toml::load_or_default(local_toml_path)?;
    toml.exclude_mcp.retain(|n| n != name);
    ace_toml::save(local_toml_path, &toml)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclude_appends_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.local.toml");
        exclude(&path, "github").unwrap();

        let toml = ace_toml::load_or_default(&path).unwrap();
        assert_eq!(toml.exclude_mcp, vec!["github".to_string()]);
    }

    #[test]
    fn exclude_dedups_on_double_add() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.local.toml");
        exclude(&path, "github").unwrap();
        exclude(&path, "github").unwrap();

        let toml = ace_toml::load_or_default(&path).unwrap();
        assert_eq!(toml.exclude_mcp, vec!["github".to_string()]);
    }

    #[test]
    fn exclude_appends_multiple_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.local.toml");
        exclude(&path, "github").unwrap();
        exclude(&path, "linear").unwrap();

        let toml = ace_toml::load_or_default(&path).unwrap();
        assert_eq!(toml.exclude_mcp, vec!["github".to_string(), "linear".to_string()]);
    }

    #[test]
    fn include_removes_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.local.toml");
        exclude(&path, "github").unwrap();
        exclude(&path, "linear").unwrap();
        include(&path, "github").unwrap();

        let toml = ace_toml::load_or_default(&path).unwrap();
        assert_eq!(toml.exclude_mcp, vec!["linear".to_string()]);
    }

    #[test]
    fn include_idempotent_on_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.local.toml");
        include(&path, "ghost").unwrap();

        let toml = ace_toml::load_or_default(&path).unwrap();
        assert!(toml.exclude_mcp.is_empty());
    }

    #[test]
    fn exclude_preserves_other_fields() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ace.local.toml");
        std::fs::write(&path, "school = \"foo/bar\"\n").unwrap();
        exclude(&path, "github").unwrap();

        let toml = ace_toml::load_or_default(&path).unwrap();
        assert_eq!(toml.school, "foo/bar");
        assert_eq!(toml.exclude_mcp, vec!["github".to_string()]);
    }
}
