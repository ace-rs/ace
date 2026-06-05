use std::collections::{BTreeSet, HashMap, HashSet};

use clap::Subcommand;

use crate::ace::Ace;
use crate::backend::Kind;
use crate::config::ace_toml::{self, AceToml, Trust};
use crate::config::tree::Tree;
use crate::config::{ConfigKey, Scope};
use crate::config::resolve::Source;

use super::CmdError;

#[derive(Subcommand)]
pub enum Command {
    /// Print the resolved value of a config key
    Get {
        /// Key to read (school, backend, trust, resume, session_prompt, env.KEY)
        key: String,
    },
    /// Set a config value in the appropriate layer
    Set {
        /// Key to write (school, backend, trust, resume, session_prompt, env.KEY)
        key: String,
        /// Value to set
        value: String,
    },
    /// Show provenance per layer for one or all keys
    Explain {
        /// Optional key to inspect (omit for all keys)
        key: Option<String>,
    },
}

pub fn run(ace: &mut Ace, command: Option<Command>) {
    let result = run_inner(ace, command);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, command: Option<Command>) -> Result<(), CmdError> {
    match command {
        None => show(ace),
        Some(Command::Get { key }) => get(ace, &key),
        Some(Command::Set { key, value }) => set(ace, &key, &value),
        Some(Command::Explain { key }) => explain(ace, key.as_deref()),
    }
}

/// Bare `ace config` — print effective resolved configuration.
///
/// Reads the merged `Resolved` only; does not bind the backend. A stale
/// `backend = "..."` selector (one not in the registry) still prints the
/// configured name without erroring — recovery is the bare `ace` command's job.
fn show(ace: &Ace) -> Result<(), CmdError> {
    let r = ace.require_resolved()?;
    let backend_name = r.backend_name.value.clone();

    let session_prompt_value = r.session_prompt.value.clone();
    let env_flat: HashMap<String, String> = r
        .env
        .iter()
        .map(|(k, v)| (k.clone(), v.value.clone()))
        .collect();
    let effective = AceToml {
        school: r.school_specifier.value.clone().unwrap_or_default(),
        backend: Some(backend_name),
        session_prompt: if session_prompt_value.is_empty() {
            None
        } else {
            Some(session_prompt_value)
        },
        env: env_flat,
        trust: r.trust.value,
        resume: if r.resume.value { None } else { Some(false) },
        skip_update: if r.skip_update.value {
            Some(true)
        } else {
            None
        },
        ..AceToml::default()
    };

    let output = toml::to_string_pretty(&effective).map_err(|e| CmdError::failed(e.to_string()))?;
    print!("{output}");

    let school_output = ace
        .school()?
        .map(toml::to_string_pretty)
        .transpose()
        .map_err(|e| CmdError::failed(e.to_string()))?;
    if let Some(s) = school_output {
        println!("\n# school.toml");
        print!("{s}");
    }

    Ok(())
}

/// `ace config get <key>` — print resolved value for a single key.
fn get(ace: &mut Ace, key: &str) -> Result<(), CmdError> {
    let config_key = ConfigKey::parse(key)
        .ok_or_else(|| CmdError::usage(format!("unknown config key: {key}")))?;

    let r = ace.require_resolved()?;

    let value = match config_key {
        ConfigKey::School => r.school_specifier.value.clone().unwrap_or_default(),
        ConfigKey::Backend => r.backend_name.value.clone(),
        ConfigKey::Trust => r.trust.value.label().to_string(),
        ConfigKey::Resume => r.resume.value.to_string(),
        ConfigKey::SkipUpdate => r.skip_update.value.to_string(),
        ConfigKey::SessionPrompt => r.session_prompt.value.clone(),
        ConfigKey::Env(ref env_key) => r
            .env
            .get(env_key)
            .map(|v| v.value.clone())
            .unwrap_or_default(),
    };

    ace.data(&value);
    Ok(())
}

/// `ace config set <key> <value>` — write a field to the appropriate layer.
fn set(ace: &mut Ace, key: &str, value: &str) -> Result<(), CmdError> {
    let config_key = ConfigKey::parse(key)
        .ok_or_else(|| CmdError::usage(format!("unknown config key: {key}")))?;

    let scope = ace
        .scope_override()
        .unwrap_or_else(|| Scope::default_for_key(config_key.scope_key()));

    let paths = ace.require_paths()?;
    let target = scope.path_in(&paths);

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut config = ace_toml::load_or_default(target)?;

    match config_key {
        ConfigKey::School => config.school = value.to_string(),
        ConfigKey::Backend => {
            let known = known_backend_names(ace.require_tree()?);
            if !known.contains(value) {
                let mut listed: Vec<&str> = known.iter().map(String::as_str).collect();
                listed.sort();
                return Err(CmdError::usage(format!(
                    "unknown backend: {value} (known: {})",
                    listed.join(", "),
                )));
            }
            config.backend = Some(value.to_string());
        }
        ConfigKey::Trust => {
            let trust = parse_trust(value)?;
            config.trust = trust;
            config.yolo = false; // clear deprecated field
        }
        ConfigKey::Resume => {
            let resume = parse_bool(value)?;
            config.resume = Some(resume);
        }
        ConfigKey::SkipUpdate => {
            config.skip_update = Some(parse_bool(value)?);
        }
        ConfigKey::SessionPrompt => {
            config.session_prompt = Some(value.to_string());
        }
        ConfigKey::Env(env_key) => {
            config.env.insert(env_key, value.to_string());
        }
    }

    ace_toml::save(target, &config)?;
    ace.done(&format!("{key} = {value}"));
    Ok(())
}

/// `ace config explain [key]` — print provenance per layer for one or all keys.
///
/// Reads the raw `Tree` and overrides directly so users can see what each layer
/// contributes — not just the merged winner. Output collapses to a single line
/// when no layer contributes (winner is `Source::Default`).
fn explain(ace: &Ace, key: Option<&str>) -> Result<(), CmdError> {
    let parsed = key
        .map(|k| {
            ConfigKey::parse(k).ok_or_else(|| CmdError::usage(format!("unknown config key: {k}")))
        })
        .transpose()?;

    let resolved = ace.require_resolved()?.clone();
    let tree = ace.require_tree()?.clone();
    let overrides = ace.overrides().clone();

    let mut blocks: Vec<String> = Vec::new();

    let want = |k: &ConfigKey| match &parsed {
        None => true,
        Some(target) => target == k,
    };

    if want(&ConfigKey::School) {
        let layers = scalar_layers(&tree, &overrides, |c| {
            if c.school.is_empty() {
                None
            } else {
                Some(c.school.clone())
            }
        });
        let winner_value = resolved.school_specifier.value.clone().unwrap_or_default();
        blocks.push(format_block(
            "school",
            &quoted(&winner_value),
            resolved.school_specifier.from,
            &layers,
            None,
        ));
    }

    if want(&ConfigKey::Backend) {
        let layers = scalar_layers(&tree, &overrides, |c| c.backend.clone());
        let school_contrib = tree
            .school
            .as_ref()
            .and_then(|s| s.backend.clone())
            .filter(|s| !s.is_empty());
        blocks.push(format_block(
            "backend",
            &quoted(&resolved.backend_name.value),
            resolved.backend_name.from,
            &layers,
            school_contrib.as_deref(),
        ));
    }

    if want(&ConfigKey::Trust) {
        let layers = scalar_layers(&tree, &overrides, |c| {
            if c.trust.is_default() && !c.yolo {
                None
            } else {
                Some(effective_trust(c).label().to_string())
            }
        });
        blocks.push(format_block(
            "trust",
            &quoted(resolved.trust.value.label()),
            resolved.trust.from,
            &layers,
            None,
        ));
    }

    if want(&ConfigKey::Resume) {
        let layers = scalar_layers(&tree, &overrides, |c| c.resume.map(|b| b.to_string()));
        blocks.push(format_block(
            "resume",
            &resolved.resume.value.to_string(),
            resolved.resume.from,
            &layers,
            None,
        ));
    }

    if want(&ConfigKey::SkipUpdate) {
        let layers = scalar_layers(&tree, &overrides, |c| c.skip_update.map(|b| b.to_string()));
        blocks.push(format_block(
            "skip_update",
            &resolved.skip_update.value.to_string(),
            resolved.skip_update.from,
            &layers,
            None,
        ));
    }

    if want(&ConfigKey::SessionPrompt) {
        let layers = scalar_layers(&tree, &overrides, |c| c.session_prompt.clone());
        blocks.push(format_block(
            "session_prompt",
            &quoted(&resolved.session_prompt.value),
            resolved.session_prompt.from,
            &layers,
            None,
        ));
    }

    let mut env_keys: BTreeSet<String> = BTreeSet::new();
    for layer in tree_layer_iter(&tree, &overrides).flatten() {
        env_keys.extend(layer.env.keys().cloned());
    }
    env_keys.extend(resolved.env.keys().cloned());
    if let Some(ConfigKey::Env(name)) = &parsed {
        // Filtered to a specific env.X — always emit a block, even when no
        // layer has it set, so the user sees an explicit "(default)" answer.
        env_keys.insert(name.clone());
    }
    for env_key in env_keys {
        let key_str = format!("env.{env_key}");
        let target = ConfigKey::Env(env_key.clone());
        if !want(&target) {
            continue;
        }
        let layers = scalar_layers(&tree, &overrides, |c| c.env.get(&env_key).cloned());
        let (winner_value, winner_from) = resolved
            .env
            .get(&env_key)
            .map(|s| (s.value.clone(), s.from))
            .unwrap_or((String::new(), Source::Default));
        blocks.push(format_block(
            &key_str,
            &quoted(&winner_value),
            winner_from,
            &layers,
            None,
        ));
    }

    if blocks.is_empty() {
        // A specific key was requested but produced nothing. Only env.* can hit
        // this path (unknown env name); other keys always exist with defaults.
        if let Some(k) = key {
            return Err(CmdError::usage(format!("unknown config key: {k}")));
        }
    }

    print!("{}", blocks.join("\n"));
    Ok(())
}

/// Per-layer values for a scalar field. Returns 4 entries: user, project, local,
/// override — in that fixed order. School is handled separately per-key (only
/// `backend` is school-contributable today).
fn scalar_layers(
    tree: &Tree,
    overrides: &AceToml,
    pick: impl Fn(&AceToml) -> Option<String>,
) -> [(Source, Option<String>); 4] {
    let user_val = tree.user.as_ref().and_then(&pick);
    let project_val = tree.project.as_ref().and_then(&pick);
    let local_val = tree.local.as_ref().and_then(&pick);
    let override_val = pick(overrides);
    [
        (Source::User, user_val),
        (Source::Project, project_val),
        (Source::Local, local_val),
        (Source::Override, override_val),
    ]
}

fn tree_layer_iter<'a>(
    tree: &'a Tree,
    overrides: &'a AceToml,
) -> impl Iterator<Item = Option<&'a AceToml>> {
    [
        tree.user.as_ref(),
        tree.project.as_ref(),
        tree.local.as_ref(),
        Some(overrides),
    ]
    .into_iter()
}

/// Build one block. `school_contrib` is the optional school-layer value (only
/// `backend` uses this slot today); when present it appears as the school row.
fn format_block(
    key: &str,
    winner_value: &str,
    winner_from: Source,
    layers: &[(Source, Option<String>); 4],
    school_contrib: Option<&str>,
) -> String {
    let any_set = layers.iter().any(|(_, v)| v.is_some()) || school_contrib.is_some();

    if !any_set {
        return format!("{key} = {winner_value}  [{}]\n", winner_from.label());
    }

    let mut out = format!("{key} = {winner_value}  [{}]\n", winner_from.label());
    let school_row = (Source::School, school_contrib.map(str::to_string));
    let rows = [&layers[0], &layers[1], &layers[2], &school_row, &layers[3]];
    for (src, val) in rows {
        let label = format!("{}:", src.label());
        let value_str = match val {
            Some(v) => quoted(v),
            None => "(unset)".to_string(),
        };
        let marker = if *src == winner_from {
            "  ← winner"
        } else {
            ""
        };
        out.push_str(&format!("  {label:<10}{value_str}{marker}\n"));
    }
    out
}

fn quoted(s: &str) -> String {
    format!("\"{s}\"")
}

/// Honour the deprecated `yolo = true` field as `Trust::Yolo` for display.
fn effective_trust(c: &AceToml) -> Trust {
    if c.yolo && c.trust.is_default() {
        Trust::Yolo
    } else {
        c.trust
    }
}

/// Names that resolve as a backend selector: built-ins + any `[[backends]]`
/// declarations across school/user/project/local layers. Used by `ace config
/// set backend` for early validation; resolve-time errors still apply.
fn known_backend_names(tree: &Tree) -> HashSet<String> {
    let mut names: HashSet<String> = Kind::ALL.iter().map(|k| k.name().to_string()).collect();
    if let Some(st) = &tree.school {
        for d in &st.backends {
            names.insert(d.name.clone());
        }
    }
    for layer in [&tree.user, &tree.project, &tree.local]
        .iter()
        .filter_map(|o| o.as_ref())
    {
        for d in &layer.backends {
            names.insert(d.name.clone());
        }
    }
    names
}

fn parse_trust(value: &str) -> Result<Trust, CmdError> {
    value.parse::<Trust>().map_err(CmdError::usage)
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
