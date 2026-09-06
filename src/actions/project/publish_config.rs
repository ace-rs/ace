//! Atomic publication shared by targeted config edits and explicit formatting.

use std::io::{Error, ErrorKind, Write};
use std::path::{Path, PathBuf};

use crate::ace::Ace;

pub struct PublishConfig<'a> {
    pub path: &'a Path,
    pub content: &'a str,
}

impl PublishConfig<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), Error> {
        let target = resolve_target(self.path)?;
        let parent = target
            .parent()
            .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "config path has no parent"))?;
        let permissions = match std::fs::metadata(&target) {
            Ok(metadata) => Some(metadata.permissions()),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };

        std::fs::create_dir_all(parent)?;
        let mut replacement = tempfile::NamedTempFile::new_in(parent)?;
        replacement.write_all(self.content.as_bytes())?;
        if let Some(permissions) = permissions {
            replacement.as_file().set_permissions(permissions)?;
        }
        replacement.as_file().sync_all()?;
        replacement.persist(&target).map_err(|error| error.error)?;

        ace.invalidate_config_caches();
        Ok(())
    }
}

fn resolve_target(path: &Path) -> Result<PathBuf, Error> {
    let mut target = path.to_path_buf();
    for _ in 0..40 {
        let metadata = match std::fs::symlink_metadata(&target) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(target),
            Err(error) => return Err(error),
        };
        if !metadata.file_type().is_symlink() {
            return Ok(target);
        }

        let link = std::fs::read_link(&target)?;
        target = if link.is_absolute() {
            link
        } else {
            let parent = target.parent().ok_or_else(|| {
                Error::new(ErrorKind::InvalidInput, "config symlink has no parent")
            })?;
            parent.join(link)
        };
    }
    Err(Error::new(
        ErrorKind::InvalidInput,
        "too many config symlink levels",
    ))
}
