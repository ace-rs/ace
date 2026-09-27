use std::io::{self, Seek, Write};

/// Materialize literal prompt text for a backend's file or stdin input surface.
pub(crate) struct StorePrompt<'a> {
    pub text: &'a str,
}

impl StorePrompt<'_> {
    pub fn run(&self) -> io::Result<tempfile::NamedTempFile> {
        let mut file = tempfile::Builder::new().prefix("ace-prompt-").tempfile()?;
        file.write_all(self.text.as_bytes())?;
        file.rewind()?;
        Ok(file)
    }
}
