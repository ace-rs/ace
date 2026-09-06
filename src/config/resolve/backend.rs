use std::collections::BTreeMap;

use crate::config::ace_toml::BackendDecl;

use super::Sourced;

/// Configured backend fields before binding validates kind and renders paths.
#[derive(Debug, Clone)]
pub struct ResolvedBackend {
    pub kind: Sourced<Option<String>>,
    pub cmd: Sourced<Vec<String>>,
    pub env: BTreeMap<String, Sourced<String>>,
    pub model: Sourced<Option<String>>,
    pub effort: Sourced<Option<String>>,
}

impl Default for ResolvedBackend {
    fn default() -> Self {
        Self {
            kind: Sourced::at_default(None),
            cmd: Sourced::at_default(Vec::new()),
            env: BTreeMap::new(),
            model: Sourced::at_default(None),
            effort: Sourced::at_default(None),
        }
    }
}

pub(super) fn merge(declarations: &[Sourced<BackendDecl>]) -> BTreeMap<String, ResolvedBackend> {
    let mut backends = BTreeMap::<String, ResolvedBackend>::new();
    for declaration in declarations {
        let configured = &declaration.value;
        let source = declaration.from;
        let backend = backends.entry(configured.name.clone()).or_default();

        if let Some(kind) = &configured.kind {
            backend.kind = Sourced::new(Some(kind.clone()), source);
        }
        if !configured.cmd.is_empty() {
            backend.cmd = Sourced::new(configured.cmd.clone(), source);
        }
        for (key, value) in &configured.env {
            backend
                .env
                .insert(key.clone(), Sourced::new(value.clone(), source));
        }
        if let Some(model) = &configured.model {
            backend.model = Sourced::new(Some(model.clone()), source);
        }
        if let Some(effort) = &configured.effort {
            backend.effort = Sourced::new(Some(effort.clone()), source);
        }
    }
    backends
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::resolve::Source;

    #[test]
    fn backend_fields_merge_independently_with_provenance() {
        let school = BackendDecl {
            name: "custom".into(),
            kind: Some("codex".into()),
            cmd: vec!["{{ school_dir }}/codex".into()],
            model: Some("school-model".into()),
            env: [
                ("KEEP".into(), "school".into()),
                ("KEY".into(), "school".into()),
            ]
            .into(),
            ..BackendDecl::default()
        };
        let local = BackendDecl {
            name: "custom".into(),
            effort: Some("high".into()),
            env: [("KEY".into(), "local".into())].into(),
            ..BackendDecl::default()
        };
        let overrides = BackendDecl {
            name: "custom".into(),
            model: Some(String::new()),
            ..BackendDecl::default()
        };
        let declarations = [
            Sourced::new(school, Source::School),
            Sourced::new(local, Source::Local),
            Sourced::new(overrides, Source::Override),
        ];

        let merged = merge(&declarations);
        let backend = &merged["custom"];

        assert_eq!(
            backend.kind,
            Sourced::new(Some("codex".into()), Source::School)
        );
        assert_eq!(
            backend.cmd,
            Sourced::new(vec!["{{ school_dir }}/codex".into()], Source::School)
        );
        assert_eq!(
            backend.model,
            Sourced::new(Some(String::new()), Source::Override)
        );
        assert_eq!(
            backend.effort,
            Sourced::new(Some("high".into()), Source::Local)
        );
        assert_eq!(
            backend.env["KEEP"],
            Sourced::new("school".into(), Source::School)
        );
        assert_eq!(
            backend.env["KEY"],
            Sourced::new("local".into(), Source::Local)
        );
    }

    #[test]
    fn incomplete_backend_remains_inspectable_without_inventing_defaults() {
        let declaration = BackendDecl {
            name: "unfinished".into(),
            model: Some("native-model".into()),
            ..BackendDecl::default()
        };

        let merged = merge(&[Sourced::new(declaration, Source::Project)]);
        let backend = &merged["unfinished"];

        assert_eq!(
            backend.model,
            Sourced::new(Some("native-model".into()), Source::Project)
        );
        assert_eq!(backend.kind, Sourced::at_default(None));
        assert_eq!(backend.cmd, Sourced::at_default(Vec::new()));
        assert_eq!(backend.effort, Sourced::at_default(None));
    }
}
