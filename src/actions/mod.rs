pub mod migrate;
pub mod project;
pub mod school;

use std::path::Path;

pub fn is_git_repo(dir: &Path) -> bool {
    dir.join(".git").exists()
}
