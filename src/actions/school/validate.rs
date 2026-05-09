use std::path::Path;

use crate::ace::Ace;
use crate::backend::registry::BackendVars;
use crate::config::school_toml;
use crate::config::ConfigError;
use crate::templates::{self, UnknownPlaceholder};

pub struct Validate<'a> {
    pub school_root: &'a Path,
}

impl Validate<'_> {
    /// Walk `school.toml` `[[backends]]`, print one issue per line via
    /// `ace.data`, return the issue count. Caller decides exit status.
    pub fn run(&self, ace: &mut Ace) -> Result<usize, ConfigError> {
        let toml = school_toml::load(&self.school_root.join("school.toml"))?;

        let mut count = 0;
        for backend in &toml.backends {
            for (i, s) in backend.cmd.iter().enumerate() {
                for u in templates::check(s, BackendVars::NAMES) {
                    ace.data(&format_issue(
                        &format!("backends[{}].cmd[{i}]", backend.name),
                        &u,
                    ));
                    count += 1;
                }
            }

            // HashMap iteration is unordered — sort env keys for stable output.
            let mut env_keys: Vec<&String> = backend.env.keys().collect();
            env_keys.sort();
            for key in env_keys {
                let value = &backend.env[key];
                for u in templates::check(value, BackendVars::NAMES) {
                    ace.data(&format_issue(
                        &format!("backends[{}].env[{key}]", backend.name),
                        &u,
                    ));
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

fn format_issue(site: &str, u: &UnknownPlaceholder) -> String {
    match &u.suggestion {
        Some(s) => format!("{site}: unknown placeholder '{}', did you mean '{}'?", u.name, s),
        None => format!("{site}: unknown placeholder '{}'", u.name),
    }
}
