use std::fs;

use clap::Subcommand;

use crate::ace::Ace;
use crate::config::skill_meta;
use crate::config::school_toml;
use crate::ace::OutputMode;
use crate::actions::school::{Init, InitError};
use crate::actions::school::{PullImports, PullImportsResult};
use crate::actions::school::Validate;

use super::CmdError;

#[derive(Subcommand)]
pub enum Command {
    /// Initialize a new school repository
    Init {
        /// School display name
        #[arg(long)]
        name: Option<String>,
        /// Overwrite existing school.toml
        #[arg(long)]
        force: bool,
    },
    /// Pull imported skills from their upstream sources
    #[command(visible_alias = "update")]
    Pull,
    /// List skills in the school
    Skills,
    /// Validate `school.toml` (typo-check `{{ ... }}` placeholders)
    Validate,
}

pub fn run(ace: &mut Ace, command: Command) {
    match command {
        Command::Init { name, force } => {
            let result = run_init(ace, name, force);
            super::exit_on_err(ace, result);
        }
        Command::Pull => {
            let result = run_pull(ace);
            super::exit_on_err(ace, result);
        }
        Command::Skills => {
            let result = run_skills(ace);
            super::exit_on_err(ace, result);
        }
        Command::Validate => {
            let result = run_validate(ace);
            super::exit_on_err(ace, result);
        }
    }
}

fn run_validate(ace: &mut Ace) -> Result<(), CmdError> {
    let school_root = ace.require_school()?.root.clone();
    let count = Validate { school_root: &school_root }.run(ace)?;
    match count {
        0 => Ok(()),
        1 => Err(CmdError::Other("1 validation issue found".into())),
        n => Err(CmdError::Other(format!("{n} validation issues found"))),
    }
}

fn run_init(ace: &mut Ace, name: Option<String>, force: bool) -> Result<(), CmdError> {
    let project_dir = ace.project_dir().to_path_buf();

    if !crate::actions::is_git_repo(&project_dir) {
        return Err(InitError::NotInGitRepo.into());
    }

    let name = match name {
        Some(n) => n,
        None => {
            let toml_path = project_dir.join("school.toml");
            let existing = if force && toml_path.exists() {
                school_toml::load(&toml_path).ok()
                    .map(|s| s.name)
                    .filter(|n| !n.is_empty())
            } else {
                None
            };
            ace.prompt_text("School name:", existing.as_deref())?
        }
    };

    Init{ name: &name, project_dir: &project_dir, force }.run(ace)?;
    Ok(())
}

fn run_skills(ace: &mut Ace) -> Result<(), CmdError> {
    let school_root = ace.require_school()?.root.clone();
    let skills_dir = school_root.join("skills");

    if !skills_dir.is_dir() {
        ace.warn("no skills directory");
        return Ok(());
    }

    let mut skills: Vec<(String, String, usize)> = Vec::new(); // (name, desc, words)
    let mut entries: Vec<_> = fs::read_dir(&skills_dir)?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        if !entry.path().is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let words = count_skill_words(&entry.path());
        match skill_meta::load(&entry.path()) {
            Some(meta) => skills.push((meta.name, meta.description, words)),
            None => skills.push((dir_name, String::new(), words)),
        }
    }

    if skills.is_empty() {
        ace.warn("no skills found");
        return Ok(());
    }

    let total_words: usize = skills.iter().map(|(_, _, w)| w).sum();
    let est_tokens = total_words * 4 / 3; // ~1.33 tokens per word

    if ace.mode() == OutputMode::Human {
        for (i, (name, desc, words)) in skills.iter().enumerate() {
            if i > 0 {
                ace.data("");
            }
            ace.data(&format!("{name} ({words}w)"));
            if !desc.is_empty() {
                ace.data(&format!("  {desc}"));
            }
        }
        ace.data(&format!(
            "\n{} skills, {total_words} words (~{est_tokens} tokens)",
            skills.len()
        ));
    } else {
        for (name, _, words) in &skills {
            ace.data(&format!("{name}\t{words}"));
        }
    }

    Ok(())
}

/// Count words across all files in a skill directory.
fn count_skill_words(skill_dir: &std::path::Path) -> usize {
    let mut total = 0;
    if let Ok(entries) = fs::read_dir(skill_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                if let Ok(content) = fs::read_to_string(&path) {
                    total += content.split_whitespace().count();
                }
            } else if path.is_dir() {
                total += count_skill_words(&path);
            }
        }
    }
    total
}

fn run_pull(ace: &mut Ace) -> Result<(), CmdError> {
    let school_root = ace.require_school()?.root.clone();

    let result = PullImports{ school_root: &school_root }.run(ace)?;
    match result {
        PullImportsResult::NoImports => ace.warn("no imports to pull"),
        PullImportsResult::Updated { .. } => {}
    }

    // Imports may have grown the school skill count — offer learn.
    crate::school::skill_count::maybe_offer_learn(ace)?;

    Ok(())
}
