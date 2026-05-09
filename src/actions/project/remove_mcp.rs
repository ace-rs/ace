use crate::ace::Ace;
use crate::backend::Backend;

pub struct RemoveMcp<'a> {
    pub backend: &'a Backend,
    pub names: &'a [String],
    pub project_dir: &'a std::path::Path,
}

impl RemoveMcp<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), String> {
        for name in self.names {
            match self.backend.mcp_remove(name, self.project_dir) {
                Ok(()) => ace.done(&format!("Removed MCP server '{name}'")),
                Err(e) => ace.warn(&format!("Failed to remove '{name}': {e}")),
            }
        }
        Ok(())
    }
}
