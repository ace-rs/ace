//! Typed configuration inspection, without backend binding or skill discovery.

use serde::Serialize;
use toml::Value;

use super::ace_toml::{AceToml, BackendDecl};
use super::resolve::{Resolved, Source, Sourced};
use super::selection::{Policy, Union};
use super::tree::Tree;
use super::{BackendConfigField, ConfigError, ConfigKey, SelectionField};
use crate::school::toml::SchoolToml;

pub struct View<'a> {
    pub resolved: &'a Resolved,
    pub tree: &'a Tree,
    pub overrides: &'a AceToml,
    pub school: Option<&'a SchoolToml>,
}

pub struct EffectiveValue {
    pub value: Value,
    pub sources: Vec<Source>,
}

impl EffectiveValue {
    fn single<T>(value: &Sourced<T>, convert: impl Fn(&T) -> Value) -> Self {
        Self {
            value: convert(&value.value),
            sources: vec![value.from],
        }
    }

    fn union(union: Union<'_>) -> Self {
        let mut sources: Vec<_> = union
            .sources()
            .into_iter()
            .filter(|(_, values)| !values.is_empty())
            .map(|(source, _)| source)
            .collect();
        if sources.is_empty() {
            sources.push(Source::Default);
        }
        Self {
            value: strings(&union.values()),
            sources,
        }
    }
}

impl View<'_> {
    pub fn effective(&self, key: &ConfigKey) -> EffectiveValue {
        let r = self.resolved;
        match key {
            ConfigKey::School => EffectiveValue::single(&r.school_specifier, optional_string),
            ConfigKey::Backend => {
                EffectiveValue::single(&r.backend_name, |v| Value::String(v.clone()))
            }
            ConfigKey::Trust => {
                EffectiveValue::single(&r.trust, |v| Value::String(v.label().into()))
            }
            ConfigKey::Resume => EffectiveValue::single(&r.resume, |v| Value::Boolean(*v)),
            ConfigKey::SkipUpdate => EffectiveValue::single(&r.skip_update, |v| Value::Boolean(*v)),
            ConfigKey::SessionPrompt => {
                EffectiveValue::single(&r.session_prompt, |v| Value::String(v.clone()))
            }
            ConfigKey::Env(name) => {
                let default = Sourced::at_default(String::new());
                EffectiveValue::single(r.env.get(name).unwrap_or(&default), |v| {
                    Value::String(v.clone())
                })
            }
            ConfigKey::BackendField { name, field } => {
                let default = Sourced::at_default(None);
                let value = r
                    .backends
                    .get(name)
                    .map(|backend| match field {
                        BackendConfigField::Model => &backend.model,
                        BackendConfigField::Effort => &backend.effort,
                    })
                    .unwrap_or(&default);
                EffectiveValue::single(value, optional_string)
            }
            ConfigKey::Selection(field) => {
                let policy = Policy::from_tree(self.tree);
                match field {
                    SelectionField::Skills => {
                        EffectiveValue::single(&policy.skills(), |v| strings(v))
                    }
                    SelectionField::IncludeSkills => EffectiveValue::union(policy.include_skills()),
                    SelectionField::ExcludeSkills => EffectiveValue::union(policy.exclude_skills()),
                    SelectionField::ExcludeMcp => EffectiveValue::union(policy.exclude_mcp()),
                }
            }
        }
    }

    pub fn keys(&self) -> Vec<ConfigKey> {
        let mut keys = Vec::from(ConfigKey::SCALARS);
        keys.extend(ConfigKey::SELECTIONS);
        let mut env_keys: Vec<_> = self.resolved.env.keys().cloned().collect();
        env_keys.sort();
        keys.extend(env_keys.into_iter().map(ConfigKey::Env));
        for name in self.resolved.backends.keys() {
            for field in [BackendConfigField::Model, BackendConfigField::Effort] {
                keys.push(ConfigKey::BackendField {
                    name: name.clone(),
                    field,
                });
            }
        }
        keys
    }

    pub fn contributions(&self, key: &ConfigKey) -> Vec<(Source, Option<Value>)> {
        let school = match (key, self.school) {
            (ConfigKey::Backend, Some(school)) => school.backend.clone().map(Value::String),
            (ConfigKey::BackendField { name, field }, Some(school)) => school
                .backends
                .get(name)
                .and_then(|backend| backend_field(backend, *field)),
            _ => None,
        };
        vec![
            (
                Source::User,
                self.tree
                    .user
                    .as_ref()
                    .and_then(|layer| raw_value(layer, key)),
            ),
            (
                Source::Project,
                self.tree
                    .project
                    .as_ref()
                    .and_then(|layer| raw_value(layer, key)),
            ),
            (
                Source::Local,
                self.tree
                    .local
                    .as_ref()
                    .and_then(|layer| raw_value(layer, key)),
            ),
            (Source::School, school),
            (Source::Override, raw_value(self.overrides, key)),
        ]
    }

    /// This is a display document, not a writable layer: defaults and empty policies
    /// are explicit, and school metadata never gets appended as a second document.
    pub fn document(&self) -> Result<toml::Table, ConfigError> {
        let mut document = toml::Table::new();
        for key in ConfigKey::SCALARS.into_iter().chain(ConfigKey::SELECTIONS) {
            document.insert(key.name(), self.effective(&key).value);
        }
        let env = self
            .resolved
            .env
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.value.clone())))
            .collect();
        document.insert("env".into(), Value::Table(env));
        let backends = self
            .resolved
            .backends
            .iter()
            .map(|(name, backend)| {
                (
                    name.clone(),
                    BackendDecl {
                        kind: backend.kind.value.clone(),
                        cmd: backend.cmd.value.clone(),
                        env: backend
                            .env
                            .iter()
                            .map(|(key, value)| (key.clone(), value.value.clone()))
                            .collect(),
                        model: backend.model.value.clone(),
                        effort: backend.effort.value.clone(),
                        ..BackendDecl::default()
                    },
                )
            })
            .collect::<std::collections::BTreeMap<_, _>>();
        document.insert("backends".into(), Value::try_from(backends)?);
        Ok(document)
    }
}

fn optional_string(value: &Option<String>) -> Value {
    Value::String(value.clone().unwrap_or_default())
}

fn strings(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}

fn backend_field(backend: &BackendDecl, field: BackendConfigField) -> Option<Value> {
    match field {
        BackendConfigField::Model => backend.model.clone(),
        BackendConfigField::Effort => backend.effort.clone(),
    }
    .map(Value::String)
}

fn raw_value(layer: &AceToml, key: &ConfigKey) -> Option<Value> {
    match key {
        ConfigKey::School => {
            (!layer.school.is_empty()).then(|| Value::String(layer.school.clone()))
        }
        ConfigKey::Backend => layer.backend.clone().map(Value::String),
        ConfigKey::Trust => layer
            .trust_override()
            .map(|v| Value::String(v.label().into())),
        ConfigKey::Resume => layer.resume.map(Value::Boolean),
        ConfigKey::SkipUpdate => layer.skip_update.map(Value::Boolean),
        ConfigKey::SessionPrompt => layer.session_prompt.clone().map(Value::String),
        ConfigKey::Env(name) => layer.env.get(name).cloned().map(Value::String),
        ConfigKey::BackendField { name, field } => layer
            .backends
            .get(name)
            .and_then(|b| backend_field(b, *field)),
        ConfigKey::Selection(field) => {
            let values = match field {
                SelectionField::Skills => &layer.skills,
                SelectionField::IncludeSkills => &layer.include_skills,
                SelectionField::ExcludeSkills => &layer.exclude_skills,
                SelectionField::ExcludeMcp => &layer.exclude_mcp,
            };
            (!values.is_empty()).then(|| strings(values))
        }
    }
}

/// Serialization for the machine-readable value returned by `config get`.
pub fn render(value: &Value) -> Result<String, ConfigError> {
    let mut output = String::new();
    value.serialize(toml::ser::ValueSerializer::new(&mut output))?;
    Ok(output)
}

/// Human-readable values keep control characters visible within a single row.
pub fn display_value(value: &Value) -> String {
    match value {
        Value::String(value) => format!("{value:?}"),
        Value::Array(values) => {
            let values = values
                .iter()
                .map(display_value)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{values}]")
        }
        value => value.to_string(),
    }
}
