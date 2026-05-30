use crate::ace::Ace;
use crate::config::{paths, school_paths};

use super::CmdError;

pub fn run(ace: &mut Ace, key: Option<&str>) {
    let result = run_inner(ace, key);
    super::exit_on_err(ace, result);
}

fn run_inner(ace: &mut Ace, key: Option<&str>) -> Result<(), CmdError> {
    let school_specifier = ace.require_resolved()?.school_specifier.value.clone();
    let p = paths::resolve(ace.project_dir())?;

    let all = build_paths(ace, &p, school_specifier.as_deref())?;

    match key {
        Some(k) => {
            let Some(value) = lookup_key(&all, k) else {
                return Err(CmdError::usage(format!("unknown key: {k}")));
            };
            ace.data(value);
        }
        None => {
            for (k, v) in &all {
                ace.data(&format!("{k}\t{v}"));
            }
        }
    }

    Ok(())
}

fn build_paths(
    ace: &Ace,
    p: &paths::AcePaths,
    school_specifier: Option<&str>,
) -> Result<Vec<(String, String)>, CmdError> {
    let mut out = Vec::new();

    out.push(("config.user".into(), p.user.display().to_string()));
    out.push(("project".into(), ace.project_dir().display().to_string()));
    out.push(("cache".into(), p.cache.display().to_string()));

    if let Some(spec) = school_specifier {
        let sp = school_paths::resolve(ace.project_dir(), spec)?;
        out.push(("school".into(), sp.root.display().to_string()));
    }

    Ok(out)
}

/// Lookup by exact key.
fn lookup_key<'a>(all: &'a [(String, String)], key: &str) -> Option<&'a str> {
    all.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}
