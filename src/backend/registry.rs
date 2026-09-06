//! Validate backend declaration kinds, render resolved configuration, and bind a name.

use std::collections::HashMap;
use std::path::Path;

use super::{Backend, BackendError, Kind, Registry};
use crate::config::ace_toml::BackendDecl;
use crate::config::resolve::Resolved;
use crate::templates::Template;

/// Render context for `{{ ... }}` placeholders inside backend `cmd` and
/// `env` values. `{{ backend_dir }}` is derived per-decl from the resolved
/// `Kind`, not carried here. See `docs/spec/backend.md § Path Templating`.
#[derive(Debug, Default, Clone)]
pub struct TemplateCtx {
    pub school_dir: String,
    pub project_dir: String,
    pub home: String,
}

#[cfg(test)]
impl TemplateCtx {
    /// All-empty context — placeholders render to empty. Test-only helper.
    pub fn empty() -> Self {
        Self::default()
    }
}

/// Per-decl placeholder catalogue: typed bag of allowed names *and* their
/// resolved values. `NAMES` and `into_map` cannot drift — adding a field
/// requires updating both at the same site. Validators read `NAMES`; the
/// renderer reads `into_map`.
pub struct BackendVars {
    pub school_dir: String,
    pub project_dir: String,
    pub home: String,
    pub backend_dir: String,
}

impl BackendVars {
    pub const NAMES: &'static [&'static str] =
        &["school_dir", "project_dir", "home", "backend_dir"];

    pub fn build(ctx: &TemplateCtx, kind: Kind) -> Self {
        let backend_dir = if ctx.project_dir.is_empty() {
            String::new()
        } else {
            format!(
                "{}/{}",
                ctx.project_dir.trim_end_matches('/'),
                kind.backend_dir()
            )
        };
        Self {
            school_dir: ctx.school_dir.clone(),
            project_dir: ctx.project_dir.clone(),
            home: ctx.home.clone(),
            backend_dir,
        }
    }

    pub fn into_map(self) -> HashMap<String, String> {
        let mut vars = HashMap::with_capacity(Self::NAMES.len());
        vars.insert("school_dir".into(), self.school_dir);
        vars.insert("project_dir".into(), self.project_dir);
        vars.insert("home".into(), self.home);
        vars.insert("backend_dir".into(), self.backend_dir);
        vars
    }
}

/// Build the registry from declarations carried on a merged `Resolved` view
/// and look up the selected backend name. Unknown name →
/// `BackendError::Unknown`.
pub fn bind(resolved: &Resolved, ctx: &TemplateCtx) -> Result<Backend, BackendError> {
    let registry = build_registry(resolved, ctx)?;
    let name = &resolved.backend_name.value;
    registry
        .lookup(name)
        .cloned()
        .ok_or_else(|| BackendError::Unknown(name.clone()))
}

/// Preserve declaration-order kind validation while consuming the single config fold.
pub fn build_registry(resolved: &Resolved, ctx: &TemplateCtx) -> Result<Registry, BackendError> {
    let mut registry = Registry::with_builtins();
    for declaration in &resolved.backend_decls {
        register_kind(&mut registry, &declaration.value)?;
    }
    for (name, configured) in &resolved.backends {
        let backend = registry
            .get_mut(name)
            .ok_or_else(|| BackendError::Unknown(name.clone()))?;
        let vars = render_vars(ctx, backend.kind);
        if !configured.cmd.value.is_empty() {
            backend.cmd = configured
                .cmd
                .value
                .iter()
                .map(|value| render(value, &vars))
                .collect();
        }
        backend.env = configured
            .env
            .iter()
            .map(|(key, value)| (key.clone(), render(&value.value, &vars)))
            .collect();
        backend.model = configured.model.value.clone();
        backend.effort = configured.effort.value.clone();
    }
    Ok(registry)
}

/// Kind is fixed by the first declaration, even when later layers replace its command.
fn register_kind(registry: &mut Registry, decl: &BackendDecl) -> Result<(), BackendError> {
    if let Some(existing) = registry.lookup(&decl.name) {
        if let Some(declared) = &decl.kind
            && Kind::from_name(declared) != Some(existing.kind)
        {
            return Err(BackendError::KindMismatch {
                name: decl.name.clone(),
                declared: declared.clone(),
                actual: existing.kind.name().to_string(),
            });
        }
        return Ok(());
    }

    let kind = resolve_kind(decl)?;
    registry.insert(Backend {
        name: decl.name.clone(),
        ..Backend::from(kind)
    });
    Ok(())
}

/// Build the placeholder map for a single decl. `backend_dir` is per-decl
/// because it depends on the resolved `Kind`. See
/// `docs/spec/backend.md § Path Templating`.
fn render_vars(ctx: &TemplateCtx, kind: Kind) -> HashMap<String, String> {
    BackendVars::build(ctx, kind).into_map()
}

/// Fast-path literal strings; only parse-and-substitute when `{{` is present.
fn render(input: &str, vars: &HashMap<String, String>) -> String {
    if !input.contains("{{") {
        return input.to_string();
    }
    Template::parse(input).substitute(vars)
}

fn resolve_kind(decl: &BackendDecl) -> Result<Kind, BackendError> {
    if let Some(declared) = &decl.kind {
        return Kind::from_name(declared)
            .ok_or_else(|| BackendError::Unresolvable(decl.name.clone()));
    }
    if let Some(k) = Kind::from_name(&decl.name) {
        return Ok(k);
    }
    if let Some(prog) = decl.cmd.first()
        && let Some(basename) = Path::new(prog).file_name().and_then(|s| s.to_str())
        && let Some(k) = Kind::from_name(basename)
    {
        return Ok(k);
    }
    Err(BackendError::Unresolvable(decl.name.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- bind() integration tests: covers merge → registry → name lookup as a
    // single pipeline. Mirrors the integration tests that lived in the
    // now-retired state/mod.rs.

    use crate::config::ace_toml::AceToml;
    use crate::config::resolve;
    use crate::config::tree::Tree;

    fn ace_with(school: &str, env: &[(&str, &str)]) -> AceToml {
        AceToml {
            school: school.to_string(),
            env: env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            ..AceToml::default()
        }
    }

    fn tree(project: AceToml, local: AceToml) -> Tree {
        Tree {
            user: None,
            project: Some(project),
            local: Some(local),
        }
    }

    fn bind_default(t: &Tree) -> Result<Backend, BackendError> {
        bind(
            &resolve::merge(t, None, &AceToml::default()),
            &TemplateCtx::empty(),
        )
    }

    fn backend_tree(project: BackendDecl, local: BackendDecl) -> Tree {
        tree(
            AceToml {
                backend: Some(project.name.clone()),
                backends: [(project.name.clone(), project)].into(),
                ..AceToml::default()
            },
            AceToml {
                backends: [(local.name.clone(), local)].into(),
                ..AceToml::default()
            },
        )
    }

    #[test]
    fn binding_uses_resolved_fields_and_retains_first_declaration_kind() {
        let project = BackendDecl {
            name: "custom".into(),
            cmd: vec!["/usr/local/bin/codex".into()],
            model: Some("model".into()),
            effort: Some("medium".into()),
            ..BackendDecl::default()
        };
        let local = BackendDecl {
            name: "custom".into(),
            cmd: vec!["wrapper".into()],
            effort: Some("high".into()),
            ..BackendDecl::default()
        };

        let backend = bind_default(&backend_tree(project, local)).expect("bind custom backend");

        assert_eq!(backend.kind, Kind::Codex);
        assert_eq!(backend.cmd, ["wrapper"]);
        assert_eq!(backend.model.as_deref(), Some("model"));
        assert_eq!(backend.effort.as_deref(), Some("high"));
    }

    #[test]
    fn later_valid_kind_does_not_hide_invalid_first_declaration() {
        let project = BackendDecl {
            name: "custom".into(),
            ..BackendDecl::default()
        };
        let local = BackendDecl {
            name: "custom".into(),
            kind: Some("codex".into()),
            ..BackendDecl::default()
        };

        let error = bind_default(&backend_tree(project, local))
            .expect_err("first declaration needs a kind");

        assert!(matches!(error, BackendError::Unresolvable(name) if name == "custom"));
    }

    #[test]
    fn later_matching_kind_does_not_hide_earlier_mismatch() {
        let project = BackendDecl {
            name: "claude".into(),
            kind: Some("codex".into()),
            ..BackendDecl::default()
        };
        let local = BackendDecl {
            name: "claude".into(),
            kind: Some("claude".into()),
            ..BackendDecl::default()
        };

        let error =
            bind_default(&backend_tree(project, local)).expect_err("all declared kinds must match");

        assert!(
            matches!(error, BackendError::KindMismatch { name, declared, actual } if name == "claude" && declared == "codex" && actual == "claude")
        );
    }

    #[test]
    fn unknown_explicit_kind_is_not_inferred_from_command() {
        let project = BackendDecl {
            name: "custom".into(),
            kind: Some("unknown".into()),
            cmd: vec!["codex".into()],
            ..BackendDecl::default()
        };
        let local = BackendDecl {
            name: "custom".into(),
            ..BackendDecl::default()
        };

        let error =
            bind_default(&backend_tree(project, local)).expect_err("explicit kind must be valid");

        assert!(matches!(error, BackendError::Unresolvable(name) if name == "custom"));
    }

    #[test]
    fn binding_renders_merged_paths_without_expanding_shell_variables() {
        let project = BackendDecl {
            name: "custom".into(),
            kind: Some("codex".into()),
            cmd: vec![
                "{{ school_dir }}/wrapper".into(),
                "{{ project_dir }}".into(),
                "{{ home }}".into(),
                "{{ backend_dir }}".into(),
                "$HOME/foo".into(),
                "~/bar".into(),
                "{{ unknown }}/x".into(),
            ],
            env: [("CFG".into(), "{{ school_dir }}/conf".into())].into(),
            ..BackendDecl::default()
        };
        let local = BackendDecl {
            name: "custom".into(),
            ..BackendDecl::default()
        };
        let tree = backend_tree(project, local);
        let resolved = resolve::merge(&tree, None, &AceToml::default());
        let context = TemplateCtx {
            school_dir: "/school".into(),
            project_dir: "/project".into(),
            home: "/home/user".into(),
        };

        let backend = bind(&resolved, &context).expect("render backend paths");

        assert_eq!(
            backend.cmd,
            [
                "/school/wrapper",
                "/project",
                "/home/user",
                "/project/.agents",
                "$HOME/foo",
                "~/bar",
                "/x"
            ]
        );
        assert_eq!(
            backend.env.get("CFG").map(String::as_str),
            Some("/school/conf")
        );
        assert_eq!(
            resolved.backends["custom"].cmd.value[0],
            "{{ school_dir }}/wrapper"
        );
    }

    #[test]
    fn bind_unknown_backend_name_errors() {
        let mut project = ace_with("s", &[]);
        project.backend = Some("nonsense".into());
        let t = tree(project, ace_with("s", &[]));
        let err = bind_default(&t).expect_err("should error");
        assert!(matches!(err, BackendError::Unknown(name) if name == "nonsense"));
    }

    #[test]
    fn bind_per_backend_env_merges_into_backend() {
        let mut project = ace_with("s", &[]);
        project.backend = Some(Kind::Claude.into());
        project.backends.insert(
            "claude".into(),
            BackendDecl {
                name: "claude".into(),
                kind: None,
                cmd: Vec::new(),
                env: [("API_BASE".to_string(), "https://example.com".to_string())]
                    .into_iter()
                    .collect(),
                model: None,
                effort: None,
                ..BackendDecl::default()
            },
        );

        let t = tree(project, ace_with("s", &[]));
        let backend = bind_default(&t).expect("bind");

        assert_eq!(backend.kind, Kind::Claude);
        assert_eq!(backend.name, "claude");
        assert_eq!(
            backend.env.get("API_BASE").map(String::as_str),
            Some("https://example.com")
        );
    }

    #[test]
    fn bind_custom_backend_selectable_by_name() {
        let mut project = ace_with("s", &[]);
        project.backend = Some("bailer".into());
        project.backends.insert(
            "bailer".into(),
            BackendDecl {
                name: "bailer".into(),
                kind: Some(Kind::Claude.into()),
                cmd: Vec::new(),
                env: [("ANTHROPIC_BASE_URL".to_string(), "https://x".to_string())]
                    .into_iter()
                    .collect(),
                model: None,
                effort: None,
                ..BackendDecl::default()
            },
        );

        let t = tree(project, ace_with("s", &[]));
        let backend = bind_default(&t).expect("bind");

        assert_eq!(backend.name, "bailer");
        assert_eq!(backend.kind, Kind::Claude);
        assert_eq!(backend.cmd, vec!["claude"]);
        assert_eq!(
            backend.env.get("ANTHROPIC_BASE_URL").map(String::as_str),
            Some("https://x")
        );
    }

    #[test]
    fn bind_per_backend_env_layer_collision_local_wins() {
        let mut project = ace_with("s", &[]);
        project.backend = Some(Kind::Claude.into());
        project.backends.insert(
            "claude".into(),
            BackendDecl {
                name: "claude".into(),
                kind: None,
                cmd: Vec::new(),
                env: [
                    ("KEEP".to_string(), "yes".to_string()),
                    ("KEY".to_string(), "old".to_string()),
                ]
                .into_iter()
                .collect(),
                model: None,
                effort: None,
                ..BackendDecl::default()
            },
        );

        let mut local = ace_with("s", &[]);
        local.backends.insert(
            "claude".into(),
            BackendDecl {
                name: "claude".into(),
                kind: None,
                cmd: Vec::new(),
                env: [("KEY".to_string(), "new".to_string())]
                    .into_iter()
                    .collect(),
                model: None,
                effort: None,
                ..BackendDecl::default()
            },
        );

        let t = tree(project, local);
        let backend = bind_default(&t).expect("bind");
        assert_eq!(backend.env.get("KEY").map(String::as_str), Some("new"));
        assert_eq!(backend.env.get("KEEP").map(String::as_str), Some("yes"));
    }
}
