use std::ffi::OsStr;
use std::path::Path;

/// Source-repo metadata that must never cross into a skill destination during a
/// copy. A nested `.git` makes the host repo record the dir as a gitlink (an
/// accidental submodule); `.gitmodules` injects phantom submodule declarations.
/// Other dot-entries (`.gitignore`, `.gitattributes`, `.editorconfig`) are
/// legitimate skill content and are preserved.
const VCS_METADATA: &[&str] = &[".git", ".gitmodules"];

fn is_vcs_metadata(name: &OsStr) -> bool {
    name.to_str().is_some_and(|n| VCS_METADATA.contains(&n))
}

/// Copy `src` into `dst` (creating `dst`), skipping VCS metadata at every level
/// so a cloned source repo never poisons the destination with an accidental
/// submodule.
pub fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        if is_vcs_metadata(&name) {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&name);
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

/// Replace `dst` with a fresh copy of `src`. Returns whether `dst` already
/// existed, so callers can classify Added vs Modified. Removing first guarantees
/// no stale entries survive from an earlier copy.
pub fn replace_dir_recursive(src: &Path, dst: &Path) -> Result<bool, std::io::Error> {
    let existed = dst.exists();
    if existed {
        std::fs::remove_dir_all(dst)?;
    }
    copy_dir_recursive(src, dst)?;
    Ok(existed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tmp() -> tempfile::TempDir {
        tempfile::tempdir().expect("tempdir")
    }

    #[test]
    fn skips_git_dir_and_gitmodules() {
        let src = tmp();
        let dst = tmp();
        fs::create_dir(src.path().join(".git")).unwrap();
        fs::write(src.path().join(".gitmodules"), "submodule").unwrap();
        fs::write(src.path().join("SKILL.md"), "# skill").unwrap();

        copy_dir_recursive(src.path(), dst.path()).unwrap();

        assert!(!dst.path().join(".git").exists());
        assert!(!dst.path().join(".gitmodules").exists());
        assert!(dst.path().join("SKILL.md").exists());
    }

    #[test]
    fn preserves_other_dotfiles() {
        let src = tmp();
        let dst = tmp();
        fs::write(src.path().join(".gitignore"), "node_modules/\n").unwrap();
        fs::write(src.path().join(".gitattributes"), "*.txt text\n").unwrap();

        copy_dir_recursive(src.path(), dst.path()).unwrap();

        assert!(dst.path().join(".gitignore").exists());
        assert!(dst.path().join(".gitattributes").exists());
    }

    #[test]
    fn skips_nested_git() {
        let src = tmp();
        let dst = tmp();
        let sub = src.path().join("sub");
        fs::create_dir_all(sub.join(".git")).unwrap();
        fs::write(sub.join("file.txt"), "data").unwrap();

        copy_dir_recursive(src.path(), dst.path()).unwrap();

        assert!(!dst.path().join("sub/.git").exists());
        assert!(dst.path().join("sub/file.txt").exists());
    }

    #[test]
    fn replace_reports_existed_and_clears_stale() {
        let src = tmp();
        let dst = tmp();
        fs::write(src.path().join("new.txt"), "new").unwrap();
        let dest = dst.path().join("skill");
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("stale.txt"), "stale").unwrap();

        let existed = replace_dir_recursive(src.path(), &dest).unwrap();

        assert!(existed);
        assert!(dest.join("new.txt").exists());
        assert!(!dest.join("stale.txt").exists());
    }

    #[test]
    fn replace_reports_absent() {
        let src = tmp();
        let dst = tmp();
        fs::write(src.path().join("a.txt"), "a").unwrap();

        let existed = replace_dir_recursive(src.path(), &dst.path().join("fresh")).unwrap();

        assert!(!existed);
    }
}
