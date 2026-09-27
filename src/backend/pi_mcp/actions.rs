use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde_json::{Map, Value};

use super::Document;
use crate::backend::{Invocation, OneShotRequest, PromptInput};

pub(super) struct RunMcpCheck {
    pub prompt: String,
}

impl RunMcpCheck {
    pub(super) fn run(&self, context: &Invocation) -> Result<String, String> {
        let options =
            crate::backend::pi::launch_options(context).map_err(|error| error.to_string())?;
        if options.mode != crate::backend::pi::OutputMode::Text {
            return Err(
                "Pi MCP health checks require --mode text; remove the incompatible --mode option"
                    .into(),
            );
        }
        let request = OneShotRequest {
            prompt: PromptInput::Stdin,
            project_dir: context.project_dir.clone(),
            env: context.env.clone(),
            extra_args: context.extra_args.clone(),
        };
        let args = crate::backend::pi::build_one_shot_args(
            context.model.as_deref(),
            context.effort.as_deref(),
            &request,
        );
        let input = crate::session::StorePrompt { text: &self.prompt }
            .run()
            .map_err(|error| format!("prepare MCP health prompt: {error}"))?;
        let stdin = input
            .reopen()
            .map_err(|error| format!("open MCP health prompt: {error}"))?;

        let _supervision = crate::platform::begin_child_supervision();
        let mut child = context
            .process()
            .args(args)
            .stdin(stdin)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| format!("start Pi MCP health check: {error}"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Pi MCP health output pipe is unavailable")?;
        let output = stream_output(stdout);
        let status = child
            .wait()
            .map_err(|error| format!("wait for Pi MCP health check: {error}"))?;
        let output = output?;
        if !status.success() {
            return Err(format!(
                "Pi MCP health check exited with {status}; inspect the visible Pi output"
            ));
        }
        String::from_utf8(output)
            .map_err(|error| format!("Pi MCP health output is not UTF-8: {error}"))
    }
}

fn stream_output(mut input: impl Read) -> Result<Vec<u8>, String> {
    let mut captured = Vec::new();
    let mut buffer = [0; 8192];
    let mut terminal = std::io::stdout().lock();
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| format!("read Pi MCP health output: {error}"))?;
        if count == 0 {
            return Ok(captured);
        }
        terminal
            .write_all(&buffer[..count])
            .map_err(|error| format!("display Pi MCP health output: {error}"))?;
        terminal
            .flush()
            .map_err(|error| format!("flush Pi MCP health output: {error}"))?;
        captured.extend_from_slice(&buffer[..count]);
    }
}

pub(super) struct WriteAdapterConfig {
    pub body: Map<String, Value>,
}

impl WriteAdapterConfig {
    pub(super) fn run(&self, document: &Document) -> Result<(), String> {
        let target = resolve_target(&document.path)?;
        let current = match std::fs::read_to_string(&target) {
            Ok(content) => Some(content),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(format!("read {}: {error}", target.display())),
        };
        if current != document.original {
            return Err(format!(
                "{} changed during MCP registration; retry without overwriting concurrent changes",
                target.display()
            ));
        }
        let parent = target
            .parent()
            .ok_or("MCP configuration path has no parent")?;
        let permissions = match std::fs::metadata(&target) {
            Ok(metadata) => Some(metadata.permissions()),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => return Err(format!("inspect {}: {error}", target.display())),
        };
        let mut content = serde_json::to_string_pretty(&self.body)
            .map_err(|error| format!("serialize MCP configuration: {error}"))?;
        content.push('\n');

        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create {}: {error}", parent.display()))?;
        let mut replacement = tempfile::NamedTempFile::new_in(parent)
            .map_err(|error| format!("prepare {}: {error}", target.display()))?;
        replacement
            .write_all(content.as_bytes())
            .map_err(|error| format!("write {}: {error}", target.display()))?;
        if let Some(permissions) = permissions {
            replacement
                .as_file()
                .set_permissions(permissions)
                .map_err(|error| {
                    format!("preserve permissions for {}: {error}", target.display())
                })?;
        }
        replacement
            .as_file()
            .sync_all()
            .map_err(|error| format!("flush {}: {error}", target.display()))?;
        replacement
            .persist(&target)
            .map_err(|error| format!("publish {}: {}", target.display(), error.error))?;
        Ok(())
    }
}

fn resolve_target(path: &Path) -> Result<PathBuf, String> {
    let mut target = path.to_path_buf();
    for _ in 0..40 {
        let metadata = match std::fs::symlink_metadata(&target) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(target),
            Err(error) => return Err(format!("inspect {}: {error}", target.display())),
        };
        if !metadata.is_symlink() {
            return Ok(target);
        }
        let link = std::fs::read_link(&target)
            .map_err(|error| format!("read link {}: {error}", target.display()))?;
        target = if link.is_absolute() {
            link
        } else {
            target
                .parent()
                .ok_or("MCP configuration symlink has no parent")?
                .join(link)
        };
    }
    Err(format!(
        "too many MCP configuration symlinks at {}",
        path.display()
    ))
}
