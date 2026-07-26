use crate::ace::{Ace, OutputMode};
use crate::actions::school::{AddImport, AddImportResult};
use crate::config::school_toml::{self, ImportDecl};
use crate::git;
use crate::skills::name;

use super::CmdError;

pub fn run(
    ace: &mut Ace,
    source: &str,
    skill: Option<&str>,
    all: bool,
    include_experimental: bool,
    include_system: bool,
) {
    let result = run_inner(
        ace,
        source,
        skill,
        all,
        include_experimental,
        include_system,
    );
    super::exit_on_err(ace, result);
}

fn run_inner(
    ace: &mut Ace,
    source: &str,
    skill: Option<&str>,
    all: bool,
    include_experimental: bool,
    include_system: bool,
) -> Result<(), CmdError> {
    if (include_experimental || include_system) && !all {
        return Err(CmdError::usage(
            "--include-experimental / --include-system require --all",
        ));
    }

    let normalized = git::normalize_source(source);
    let school_root = ace.require_school()?.root.clone();

    // --all is shorthand for --skill "*"
    let effective_skill = if all { Some("*") } else { skill };

    // Glob patterns are recorded as imports, not copied immediately.
    // They resolve on `ace school pull`.
    if let Some(pattern) = effective_skill
        && crate::glob::is_glob(pattern)
    {
        return add_glob_import(
            ace,
            &school_root,
            &normalized,
            pattern,
            include_experimental,
            include_system,
        );
    }

    let result = AddImport {
        source: &normalized,
        skill: effective_skill,
        school_root: &school_root,
    }
    .run(ace)?;

    match result {
        AddImportResult::Done => {}
        AddImportResult::NeedsSelection(skills) => {
            // Picks come back as indices, so labels are free to be the
            // sanitized display form rather than the verbatim identity.
            let labels = skills
                .iter()
                .map(|s| name::render(&s.locator).to_string())
                .collect();
            let picked = ace.prompt_multiselect("Pick skills to import", labels, false)?;

            // An empty pick means two different things. Interactively the user
            // declined, which is a valid outcome. Without a terminal there was
            // no picker at all, and exiting 0 would report success for an
            // import that never happened.
            if picked.is_empty() {
                if ace.mode() != OutputMode::Human {
                    return Err(CmdError::usage(
                        "multiple skills found and no terminal to pick from",
                    )
                    .with_hint("pass `--skill <name>` or `--all`"));
                }

                ace.info("no skills selected");
                return Ok(());
            }

            let import = AddImport {
                source: &normalized,
                skill: None,
                school_root: &school_root,
            };
            // One bad skill must not strand the rest of the batch — report it
            // and carry on, failing only if nothing at all landed.
            let mut installed = 0;
            for (_, skill) in skills
                .iter()
                .enumerate()
                .filter(|(i, _)| picked.contains(i))
            {
                match import.install_selected(skill, ace) {
                    Ok(()) => installed += 1,
                    Err(e) => ace.warn(&format!("{}: {e}", name::render(&skill.locator))),
                }
            }

            if installed == 0 {
                return Err(CmdError::failed("no skills were imported"));
            }
        }
    }
    Ok(())
}

/// Record a glob import entry in school.toml without copying any skills.
/// Skills matching the pattern are resolved during `ace school pull`.
fn add_glob_import(
    ace: &mut Ace,
    school_root: &std::path::Path,
    source: &str,
    pattern: &str,
    include_experimental: bool,
    include_system: bool,
) -> Result<(), CmdError> {
    let toml_path = school_root.join("school.toml");
    let mut school = school_toml::load(&toml_path)?;

    let entry = school
        .imports
        .iter_mut()
        .find(|i| i.patterns() == vec![pattern] && i.source == source);

    if entry.is_some() {
        ace.warn(&format!("import already exists: {pattern} from {source}"));
        return Ok(());
    }

    school.imports.push(ImportDecl {
        source: source.to_string(),
        skills: vec![pattern.to_string()],
        include_experimental,
        include_system,
        ..ImportDecl::default()
    });

    school_toml::save(&toml_path, &school)?;
    ace.done(&format!("Added import: {pattern} from {source}"));
    ace.hint("Run 'ace school pull' to fetch matching skills");
    Ok(())
}
