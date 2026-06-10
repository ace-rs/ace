use std::path::Path;

use crate::ace::Ace;
use crate::skills::name;

/// Names whose destination `skills/<name>` is committed as a gitlink — an
/// accidental submodule left by an earlier import that leaked a `.git`. Such a
/// path can never be written as plain files until the gitlink is cleared from
/// the index, so callers skip these and warn.
pub fn gitlinked_names(school_root: &Path, names: &[String]) -> Vec<String> {
    let gitlinks = crate::git::gitlinks_under(school_root, "skills");
    if gitlinks.is_empty() {
        return Vec::new();
    }

    names
        .iter()
        .filter(|name| gitlinks.contains(&Path::new("skills").join(name.as_str())))
        .cloned()
        .collect()
}

/// Warn that a skill is a broken submodule and point at the one-line fix. ACE
/// deliberately does not run the fix itself — the index is the user's to
/// rewrite, and they may be mid-surgery on it.
pub fn warn_broken_submodule(ace: &mut Ace, skill: &str) {
    ace.warn(&format!(
        "skill `{}` is committed as a git submodule at skills/{skill} — skipping; \
         an earlier import leaked a .git directory here",
        name::render(skill),
    ));
    ace.hint(&format!(
        "run `git rm --cached skills/{skill}` then re-run `ace school pull`"
    ));
}
