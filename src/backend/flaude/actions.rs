use std::io::Write;

use serde_json::Value;

use crate::backend::Invocation;

pub(super) struct RecordCall<'a> {
    pub action: &'a str,
    pub names: &'a [String],
}

impl RecordCall<'_> {
    pub(super) fn run(&self, context: &Invocation) -> Result<(), String> {
        let path = context.home_dir()?.join(".flaude-calls.jsonl");
        let record: Value = serde_json::json!({
            "action": self.action,
            "names": self.names,
            "cmd": context.command,
            "env": context.env,
            "project_dir": context.project_dir.to_string_lossy(),
            "model": context.model,
            "effort": context.effort,
            "extra_args": context.extra_args,
        });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|error| format!("open {}: {error}", path.display()))?;

        writeln!(file, "{record}").map_err(|error| format!("write {}: {error}", path.display()))
    }
}
