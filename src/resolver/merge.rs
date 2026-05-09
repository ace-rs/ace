//! Pure-logic merge: layered `Tree` + overrides → `Resolved` with provenance.
//!
//! Infallible past parse. No registry lookup, no binding I/O. Validation
//! (e.g. "backend name exists") lives in the binding layer.

use std::collections::HashMap;

use crate::backend::Kind;
use crate::config::ace_toml::{AceToml, BackendDecl, Trust};
use crate::config::tree::Tree;

use super::resolved::Resolved;
use super::source::{Source, Sourced};

pub fn merge(tree: &Tree, overrides: &AceToml) -> Resolved {
    let default = AceToml::default();
    let user = tree.user.as_ref().unwrap_or(&default);
    let project = tree.project.as_ref().unwrap_or(&default);
    let local = tree.local.as_ref().unwrap_or(&default);

    let layers: [(Source, &AceToml); 4] = [
        (Source::User, user),
        (Source::Project, project),
        (Source::Local, local),
        (Source::Override, overrides),
    ];

    // Personal-only fields skip the project layer entirely; user, local, and
    // the runtime override layer are all "personal" and contribute. See
    // docs/spec/configuration.md.
    let personal: [(Source, &AceToml); 3] = [
        (Source::User, user),
        (Source::Local, local),
        (Source::Override, overrides),
    ];

    Resolved {
        school_specifier: school_specifier(&layers),
        backend_name: backend_name(&layers, tree.school_backend()),
        backend_decls: backend_decls(tree, &layers),
        session_prompt: session_prompt(&layers),
        env: env(&layers),
        trust: trust(&personal),
        resume: resume(&personal),
        skip_update: skip_update(&layers),
    }
}

fn school_specifier(layers: &[(Source, &AceToml); 4]) -> Sourced<Option<String>> {
    // Last-wins among non-empty `school` strings.
    for (src, layer) in layers.iter().rev() {
        if !layer.school.is_empty() {
            return Sourced::new(Some(layer.school.clone()), *src);
        }
    }
    Sourced::at_default(None)
}

fn backend_name(
    layers: &[(Source, &AceToml); 4],
    school_backend: Option<&str>,
) -> Sourced<String> {
    // Override → local → project → user → school → "claude"
    for (src, layer) in layers.iter().rev() {
        if let Some(name) = &layer.backend {
            return Sourced::new(name.clone(), *src);
        }
    }
    if let Some(name) = school_backend {
        return Sourced::new(name.to_string(), Source::School);
    }
    Sourced::at_default(Kind::default().into())
}

fn backend_decls(tree: &Tree, layers: &[(Source, &AceToml); 4]) -> Vec<Sourced<BackendDecl>> {
    let mut out = Vec::new();
    if let Some(st) = &tree.school {
        for d in &st.backends {
            out.push(Sourced::new(d.clone(), Source::School));
        }
    }
    for (src, layer) in layers {
        for d in &layer.backends {
            out.push(Sourced::new(d.clone(), *src));
        }
    }
    out
}

fn session_prompt(layers: &[(Source, &AceToml); 4]) -> Sourced<String> {
    // Last Some wins (Some("") is a valid override to empty).
    for (src, layer) in layers.iter().rev() {
        if let Some(p) = &layer.session_prompt {
            return Sourced::new(p.clone(), *src);
        }
    }
    Sourced::at_default(String::new())
}

fn env(layers: &[(Source, &AceToml); 4]) -> HashMap<String, Sourced<String>> {
    // Additive merge; later layers overwrite per key. Per-entry provenance.
    let mut out: HashMap<String, Sourced<String>> = HashMap::new();
    for (src, layer) in layers {
        for (k, v) in &layer.env {
            out.insert(k.clone(), Sourced::new(v.clone(), *src));
        }
    }
    out
}

fn trust(layers: &[(Source, &AceToml)]) -> Sourced<Trust> {
    // Last non-default trust wins; backcompat yolo=true → Trust::Yolo at the
    // same layer.
    for (src, layer) in layers.iter().rev() {
        if !layer.trust.is_default() {
            return Sourced::new(layer.trust, *src);
        }
        if layer.yolo {
            return Sourced::new(Trust::Yolo, *src);
        }
    }
    Sourced::at_default(Trust::Default)
}

fn resume(layers: &[(Source, &AceToml)]) -> Sourced<bool> {
    // Last Some wins. Default true.
    for (src, layer) in layers.iter().rev() {
        if let Some(v) = layer.resume {
            return Sourced::new(v, *src);
        }
    }
    Sourced::at_default(true)
}

fn skip_update(layers: &[(Source, &AceToml); 4]) -> Sourced<bool> {
    // Standard last-Some wins across all layers.
    for (src, layer) in layers.iter().rev() {
        if let Some(v) = layer.skip_update {
            return Sourced::new(v, *src);
        }
    }
    Sourced::at_default(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ace_toml::BackendDecl;
    use crate::config::tree::Tree;

    fn ace(school: &str, env: &[(&str, &str)]) -> AceToml {
        AceToml {
            school: school.to_string(),
            env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            ..AceToml::default()
        }
    }

    fn tree(project: AceToml, local: AceToml) -> Tree {
        Tree {
            user: None,
            project: Some(project),
            local: Some(local),
            school: None,
        }
    }

    fn tree_with_school_backend(
        project: AceToml,
        local: AceToml,
        backend: &str,
    ) -> Tree {
        use crate::config::school_toml::SchoolToml;
        Tree {
            user: None,
            project: Some(project),
            local: Some(local),
            school: Some(SchoolToml {
                backend: Some(backend.to_string()),
                ..SchoolToml::default()
            }),
        }
    }

    fn empty_overrides() -> AceToml {
        AceToml::default()
    }

    #[test]
    fn school_project_wins() {
        let t = tree(ace("project-school", &[]), ace("", &[]));
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.school_specifier.value.as_deref(), Some("project-school"));
        assert_eq!(r.school_specifier.from, Source::Project);
    }

    #[test]
    fn school_local_overrides_project() {
        let t = tree(ace("project-school", &[]), ace("local-school", &[]));
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.school_specifier.value.as_deref(), Some("local-school"));
        assert_eq!(r.school_specifier.from, Source::Local);
    }

    #[test]
    fn school_default_when_all_empty() {
        let t = tree(ace("", &[]), ace("", &[]));
        let r = merge(&t, &empty_overrides());
        assert!(r.school_specifier.value.is_none());
        assert_eq!(r.school_specifier.from, Source::Default);
    }

    #[test]
    fn backend_local_overrides_project() {
        let mut project = ace("", &[]);
        project.backend = Some(Kind::Codex.into());
        let mut local = ace("", &[]);
        local.backend = Some(Kind::Claude.into());

        let t = tree(project, local);
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.backend_name.value, "claude");
        assert_eq!(r.backend_name.from, Source::Local);
    }

    #[test]
    fn backend_default_claude() {
        let t = tree(ace("", &[]), ace("", &[]));
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.backend_name.value, "claude");
        assert_eq!(r.backend_name.from, Source::Default);
    }

    #[test]
    fn backend_school_used() {
        let t = tree_with_school_backend(ace("", &[]), ace("", &[]), Kind::Codex.name());
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.backend_name.value, "codex");
        assert_eq!(r.backend_name.from, Source::School);
    }

    #[test]
    fn backend_project_overrides_school() {
        let mut project = ace("", &[]);
        project.backend = Some(Kind::Claude.into());
        let t = tree_with_school_backend(project, ace("", &[]), Kind::Codex.name());

        let r = merge(&t, &empty_overrides());
        assert_eq!(r.backend_name.value, "claude");
        assert_eq!(r.backend_name.from, Source::Project);
    }

    #[test]
    fn backend_override_beats_all_layers() {
        let mut project = ace("", &[]);
        project.backend = Some(Kind::Flaude.into());
        let mut local = ace("", &[]);
        local.backend = Some(Kind::Claude.into());
        let t = tree_with_school_backend(project, local, Kind::Codex.name());

        let overrides = AceToml { backend: Some(Kind::Codex.into()), ..AceToml::default() };
        let r = merge(&t, &overrides);
        assert_eq!(r.backend_name.value, "codex");
        assert_eq!(r.backend_name.from, Source::Override);
    }

    #[test]
    fn env_additive_with_provenance() {
        let t = tree(ace("s", &[("A", "1")]), ace("s", &[("B", "2")]));
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.env["A"].value, "1");
        assert_eq!(r.env["A"].from, Source::Project);
        assert_eq!(r.env["B"].value, "2");
        assert_eq!(r.env["B"].from, Source::Local);
    }

    #[test]
    fn env_local_overrides_project() {
        let t = tree(
            ace("s", &[("KEY", "old"), ("KEEP", "yes")]),
            ace("s", &[("KEY", "new")]),
        );
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.env["KEY"].value, "new");
        assert_eq!(r.env["KEY"].from, Source::Local);
        assert_eq!(r.env["KEEP"].value, "yes");
        assert_eq!(r.env["KEEP"].from, Source::Project);
    }

    #[test]
    fn session_prompt_last_wins() {
        let mut project = ace("", &[]);
        project.session_prompt = Some("project prompt".to_string());
        let t = tree(project, ace("", &[]));
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.session_prompt.value, "project prompt");
        assert_eq!(r.session_prompt.from, Source::Project);
    }

    #[test]
    fn session_prompt_default_empty() {
        let t = tree(ace("", &[]), ace("", &[]));
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.session_prompt.value, "");
        assert_eq!(r.session_prompt.from, Source::Default);
    }

    #[test]
    fn trust_local_wins() {
        let user = AceToml { trust: Trust::Auto, ..AceToml::default() };
        let local = AceToml { trust: Trust::Yolo, ..AceToml::default() };

        let t = Tree {
            user: Some(user),
            project: Some(AceToml::default()),
            local: Some(local),
            school: None,
        };
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.trust.value, Trust::Yolo);
        assert_eq!(r.trust.from, Source::Local);
    }

    #[test]
    fn trust_yolo_legacy_local() {
        let local = AceToml { yolo: true, ..AceToml::default() };
        let t = Tree {
            user: None,
            project: None,
            local: Some(local),
            school: None,
        };
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.trust.value, Trust::Yolo);
        assert_eq!(r.trust.from, Source::Local);
    }

    #[test]
    fn trust_default_when_unset() {
        let t = tree(ace("", &[]), ace("", &[]));
        let r = merge(&t, &empty_overrides());
        assert_eq!(r.trust.value, Trust::Default);
        assert_eq!(r.trust.from, Source::Default);
    }

    #[test]
    fn resume_default_true() {
        let t = tree(ace("s", &[]), ace("s", &[]));
        let r = merge(&t, &empty_overrides());
        assert!(r.resume.value);
        assert_eq!(r.resume.from, Source::Default);
    }

    #[test]
    fn resume_local_false_disables() {
        let mut local = ace("s", &[]);
        local.resume = Some(false);
        let t = tree(ace("s", &[]), local);
        let r = merge(&t, &empty_overrides());
        assert!(!r.resume.value);
        assert_eq!(r.resume.from, Source::Local);
    }

    #[test]
    fn resume_project_ignored() {
        // Personal-only: project-level resume is silently dropped.
        let mut project = ace("s", &[]);
        project.resume = Some(false);
        let t = tree(project, ace("s", &[]));
        let r = merge(&t, &empty_overrides());
        assert!(r.resume.value);
        assert_eq!(r.resume.from, Source::Default);
    }

    #[test]
    fn skip_update_defaults_false() {
        let t = tree(ace("s", &[]), ace("s", &[]));
        let r = merge(&t, &empty_overrides());
        assert!(!r.skip_update.value);
        assert_eq!(r.skip_update.from, Source::Default);
    }

    #[test]
    fn skip_update_local_false_overrides_project_true() {
        let mut project = ace("s", &[]);
        project.skip_update = Some(true);
        let mut local = ace("s", &[]);
        local.skip_update = Some(false);
        let t = tree(project, local);
        let r = merge(&t, &empty_overrides());
        assert!(!r.skip_update.value);
        assert_eq!(r.skip_update.from, Source::Local);
    }

    #[test]
    fn skip_update_user_propagates_when_others_unset() {
        let t = Tree {
            user: Some(AceToml { skip_update: Some(true), ..AceToml::default() }),
            project: None,
            local: None,
            school: None,
        };
        let r = merge(&t, &empty_overrides());
        assert!(r.skip_update.value);
        assert_eq!(r.skip_update.from, Source::User);
    }

    #[test]
    fn backend_decls_collected_with_provenance() {
        let mut project = ace("", &[]);
        project.backends = vec![BackendDecl {
            name: "p".into(),
            kind: None,
            cmd: Vec::new(),
            env: HashMap::new(),
        }];
        let mut local = ace("", &[]);
        local.backends = vec![BackendDecl {
            name: "l".into(),
            kind: None,
            cmd: Vec::new(),
            env: HashMap::new(),
        }];

        let t = tree(project, local);
        let r = merge(&t, &empty_overrides());

        assert_eq!(r.backend_decls.len(), 2);
        assert_eq!(r.backend_decls[0].value.name, "p");
        assert_eq!(r.backend_decls[0].from, Source::Project);
        assert_eq!(r.backend_decls[1].value.name, "l");
        assert_eq!(r.backend_decls[1].from, Source::Local);
    }
}
