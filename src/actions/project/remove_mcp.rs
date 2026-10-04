use crate::ace::Ace;
use crate::backend::{Invocation, McpError, McpRemoval};

pub struct RemoveMcp<'a> {
    pub invocation: &'a Invocation,
    pub names: &'a [String],
}

impl RemoveMcp<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), McpError> {
        for name in self.names {
            match self.invocation.mcp_remove(name) {
                Ok(McpRemoval::Removed) => ace.done(&format!("Removed MCP server '{name}'")),
                Ok(McpRemoval::StillRegistered { source }) => ace.warn(&format!(
                    "MCP server '{name}' remains registered by {}; edit that source to remove it",
                    source.display(),
                )),
                Err(error) => return Err(error.at_server(name)),
            }
        }
        Ok(())
    }
}
