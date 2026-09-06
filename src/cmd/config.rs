use clap::Subcommand;
use toml::Value;

use crate::ace::Ace;
use crate::actions::project::edit_config::{EditConfig, FieldEdit};
use crate::backend::Kind;
use crate::config::ace_toml::Trust;
use crate::config::inspection::{View, display_value, render};
use crate::config::resolve::Source;
use crate::config::{ConfigError, ConfigKey, Scope};

use super::CmdError;

#[derive(Subcommand)]
pub enum Command {
    /// Print the resolved value of a config key
    Get {
        #[arg(help = ConfigKey::help())]
        key: String,
    },
    /// Set a config value in the appropriate layer
    Set {
        #[arg(help = ConfigKey::help())]
        key: String,
        /// Value to set
        value: String,
    },
    /// Show provenance per layer for one or all keys
    Explain {
        #[arg(help = ConfigKey::help())]
        key: Option<String>,
    },
}

pub fn run(ace: &mut Ace, command: Option<Command>) {
    let result = run_inner(ace, command);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, command: Option<Command>) -> Result<(), CmdError> {
    match command {
        None => {
            report_diagnostics(ace)?;
            show(ace)
        }
        Some(Command::Get { key }) => {
            let key = parse_key(&key)?;
            report_diagnostics(ace)?;
            get(ace, &key)
        }
        Some(Command::Set { key, value }) => set(ace, &parse_key(&key)?, &value),
        Some(Command::Explain { key }) => {
            let key = key.as_deref().map(parse_key).transpose()?;
            report_diagnostics(ace)?;
            explain(ace, key)
        }
    }
}

fn parse_key(key: &str) -> Result<ConfigKey, CmdError> {
    ConfigKey::parse(key).ok_or_else(|| CmdError::usage(format!("unknown config key: {key}")))
}

fn view(ace: &Ace) -> Result<View<'_>, CmdError> {
    let resolved = ace.require_config()?;
    let school = match ace.school_toml() {
        Ok(school) => Some(school),
        Err(error) if error.is_absent() => None,
        Err(error) => return Err(error.into()),
    };
    Ok(View {
        resolved,
        tree: ace.require_tree()?,
        overrides: ace.overrides(),
        school,
    })
}

fn show(ace: &Ace) -> Result<(), CmdError> {
    let document = view(ace)?.document()?;
    let output = toml::to_string_pretty(&document).map_err(ConfigError::from)?;
    print!("{output}");
    Ok(())
}

fn get(ace: &mut Ace, key: &ConfigKey) -> Result<(), CmdError> {
    let effective = view(ace)?.effective(key);
    let output = match effective.value {
        Value::String(value) => value,
        value => render(&value)?,
    };
    ace.data(&output);
    Ok(())
}

fn set(ace: &mut Ace, key: &ConfigKey, value: &str) -> Result<(), CmdError> {
    if !key.is_writable() {
        return Err(CmdError::usage(format!(
            "config key is read-only: {}",
            key.name()
        )));
    }
    let scope = ace
        .scope_override()
        .unwrap_or_else(|| Scope::default_for_key(key.scope_key()));
    let target = scope.path_in(ace.paths()).to_path_buf();
    let assignment = match key {
        ConfigKey::School => FieldEdit::new("school", value),
        ConfigKey::Backend => {
            validate_backend(ace, value)?;
            FieldEdit::new("backend", value)
        }
        ConfigKey::Trust => FieldEdit::new(
            "trust",
            value.parse::<Trust>().map_err(CmdError::usage)?.label(),
        ),
        ConfigKey::Resume => FieldEdit::new("resume", parse_bool(value)?),
        ConfigKey::SkipUpdate => FieldEdit::new("skip_update", parse_bool(value)?),
        ConfigKey::SessionPrompt => FieldEdit::new("session_prompt", value),
        ConfigKey::Env(name) => FieldEdit::new(name, value).in_tables(["env".to_string()]),
        ConfigKey::BackendField { name, field } => {
            FieldEdit::new(field.label(), value).in_tables(["backends".to_string(), name.clone()])
        }
        ConfigKey::Selection(_) => return Err(CmdError::usage("selection fields are read-only")),
    };
    let assignments = if matches!(key, ConfigKey::Trust) {
        vec![assignment, FieldEdit::remove("yolo")]
    } else {
        vec![assignment]
    };
    EditConfig {
        path: &target,
        assignments,
    }
    .run(ace)?;
    ace.done(&format!(
        "saved {} in {} ({})",
        key.display_name(),
        scope.label(),
        target.display()
    ));

    // Publication has succeeded; an inspection failure cannot undo that fact or
    // turn the successful write into a misleading command failure.
    if let Err(error) = report_write_effect(ace, key, scope) {
        ace.warn(&format!(
            "saved configuration; effective value could not be inspected: {error}"
        ));
    }
    if let Err(error) = report_diagnostics(ace) {
        ace.warn(&format!(
            "saved configuration; field diagnostics unavailable: {error}"
        ));
    }
    Ok(())
}

fn validate_backend(ace: &Ace, name: &str) -> Result<(), CmdError> {
    // Built-ins do not require a pre-existing configuration or a linked school.
    if Kind::from_name(name).is_some() {
        return Ok(());
    }
    let known = match ace.known_backend_names() {
        Ok(names) => names,
        Err(ConfigError::NoConfig) => Kind::ALL
            .iter()
            .map(|kind| kind.name().to_string())
            .collect(),
        Err(error) => return Err(error.into()),
    };
    if known.iter().any(|known| known == name) {
        return Ok(());
    }
    Err(CmdError::usage(format!(
        "unknown backend: {name} (known: {})",
        known.join(", ")
    )))
}

fn report_write_effect(ace: &mut Ace, key: &ConfigKey, scope: Scope) -> Result<(), CmdError> {
    let effective = view(ace)?.effective(key);
    let value = display_value(&effective.value);
    let target_source = match scope {
        Scope::User => Source::User,
        Scope::Project => Source::Project,
        Scope::Local => Source::Local,
    };
    let consequence = if key.is_personal() && scope == Scope::Project {
        "ignored: personal-only"
    } else if !effective.sources.contains(&target_source) {
        "overridden or non-contributing"
    } else {
        "effective"
    };
    let sources = effective
        .sources
        .iter()
        .map(|source| source.label())
        .collect::<Vec<_>>()
        .join(", ");
    ace.info(&format!(
        "{consequence}: {} = {value} [{sources}]",
        key.display_name()
    ));
    Ok(())
}

fn explain(ace: &Ace, key: Option<ConfigKey>) -> Result<(), CmdError> {
    let view = view(ace)?;
    let keys = key.map(|key| vec![key]).unwrap_or_else(|| view.keys());
    let blocks = keys
        .iter()
        .map(|key| format_block(&view, key))
        .collect::<Vec<_>>();
    print!("{}", blocks.join("\n"));
    Ok(())
}

fn format_block(view: &View<'_>, key: &ConfigKey) -> String {
    let effective = view.effective(key);
    let value = display_value(&effective.value);
    let sources = effective
        .sources
        .iter()
        .map(|source| source.label())
        .collect::<Vec<_>>()
        .join(", ");
    let mut output = format!("{} = {value}  [{sources}]\n", key.display_name());
    let rows = view.contributions(key);
    if rows.iter().all(|(_, value)| value.is_none()) {
        return output;
    }

    for (source, value) in rows {
        let marker = if source == Source::Project && key.is_personal() && value.is_some() {
            "  (ignored: personal-only)"
        } else if effective.sources.contains(&source) && key.is_union() {
            "  ← contributor"
        } else if effective.sources.contains(&source) {
            "  ← winner"
        } else {
            ""
        };
        let rendered = match value {
            Some(value) => display_value(&value),
            None => "(unset)".into(),
        };
        let label = format!("{}:", source.label());
        output.push_str(&format!("  {label:<10}{rendered}{marker}\n"));
    }
    output
}

fn report_diagnostics(ace: &mut Ace) -> Result<(), CmdError> {
    let tree = ace.require_tree()?;
    let scopes = [
        (Scope::User, &tree.user),
        (Scope::Project, &tree.project),
        (Scope::Local, &tree.local),
    ];
    let mut messages = Vec::new();
    for (scope, layer) in scopes {
        let Some(layer) = layer else {
            continue;
        };
        for key in layer.unknown_field_paths() {
            let path = scope.path_in(ace.paths());
            messages.push(format!(
                "{}: {key}: unknown or misplaced field; ignored",
                path.display()
            ));
        }
    }
    match ace.school_toml() {
        Ok(school) => {
            let path = ace.require_linked_school()?.toml_path();
            let unknown_fields = school
                .backends
                .iter()
                .flat_map(|(name, backend)| backend.unknown_field_paths(name));
            for key in unknown_fields {
                messages.push(format!(
                    "{}: {key}: unknown or misplaced field; ignored",
                    path.display()
                ));
            }
        }
        Err(error) if error.is_absent() => {}
        Err(error) => return Err(error.into()),
    }
    for message in messages {
        ace.warn(&message);
    }
    Ok(())
}

fn parse_bool(value: &str) -> Result<bool, CmdError> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(CmdError::usage(format!(
            "expected true or false, got: {value}"
        ))),
    }
}
