use crate::ace::Ace;
use crate::actions::school::{AddImport, AddImportError, AddImportResult};
use crate::config::school_toml::{self, ImportDecl};
use crate::git;

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

    let normalized = git::normalize_github_source(source);
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
        AddImportResult::Done { .. } => {}
        AddImportResult::NeedsSelection(skills) => {
            let names: Vec<String> = skills.iter().map(|s| s.locator.to_string()).collect();
            let selected = ace.prompt_select("Multiple skills found, pick one:", names)?;

            let skill = skills
                .iter()
                .find(|s| s.locator == selected.as_str())
                .ok_or_else(|| AddImportError::SkillNotFound(selected.to_string()))?;

            AddImport {
                source: &normalized,
                skill: Some(skill.locator.as_str()),
                school_root: &school_root,
            }
            .install_selected(skill, ace)?;
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
