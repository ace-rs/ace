use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};

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
        let output = stream_output(stdout, std::io::stdout().lock());
        let status = child
            .wait()
            .map_err(|error| format!("wait for Pi MCP health check: {error}"))?;
        check_output(output?, status)
    }
}

fn stream_output(mut input: impl Read, mut display: impl Write) -> Result<Vec<u8>, String> {
    let mut captured = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|error| format!("read Pi MCP health output: {error}"))?;
        if count == 0 {
            return Ok(captured);
        }
        display
            .write_all(&buffer[..count])
            .map_err(|error| format!("display Pi MCP health output: {error}"))?;
        display
            .flush()
            .map_err(|error| format!("flush Pi MCP health output: {error}"))?;
        captured.extend_from_slice(&buffer[..count]);
    }
}

fn check_output(output: Vec<u8>, status: ExitStatus) -> Result<String, String> {
    if !status.success() {
        return Err(format!(
            "Pi MCP health check exited with {status}; inspect the visible Pi output"
        ));
    }

    String::from_utf8(output).map_err(|error| format!("Pi MCP health output is not UTF-8: {error}"))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    #[test]
    fn live_health_output_is_displayed_and_retained_across_buffer_boundaries() {
        let output = "connection progress\n".repeat(1200).into_bytes();
        let mut displayed = Vec::new();

        let captured =
            stream_output(Cursor::new(&output), &mut displayed).expect("stream all live output");

        assert_eq!(displayed, output);
        assert_eq!(captured, output);
    }

    #[cfg(unix)]
    fn status(code: i32) -> ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        ExitStatus::from_raw(code << 8)
    }

    #[cfg(unix)]
    #[test]
    fn failed_health_process_cannot_pass_with_an_otherwise_successful_report() {
        let report = br#"{"statuses":[{"name":"example","ok":true}]}"#;
        let mut displayed = Vec::new();
        let captured =
            stream_output(Cursor::new(report), &mut displayed).expect("display process output");

        let error = check_output(captured, status(7)).expect_err("nonzero health process");

        assert_eq!(displayed, report);
        assert!(error.contains(&status(7).to_string()), "{error}");
        assert!(error.contains("inspect the visible Pi output"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn successful_health_process_preserves_report_and_rejects_non_utf8_after_display() {
        let report = r#"{"statuses":[{"name":"example","ok":true}]}"#;
        assert_eq!(
            check_output(report.as_bytes().into(), status(0)).expect("successful output"),
            report
        );

        let bytes = b"connection failed: \xff";
        let mut displayed = Vec::new();
        let captured =
            stream_output(Cursor::new(bytes), &mut displayed).expect("display diagnostic bytes");
        let error = check_output(captured, status(0)).expect_err("invalid UTF-8");

        assert_eq!(displayed, bytes);
        assert!(error.contains("not UTF-8"), "{error}");
    }

    struct FailedRead;

    impl Read for FailedRead {
        fn read(&mut self, _buffer: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("pipe read failed"))
        }
    }

    #[test]
    fn read_failure_keeps_prior_diagnostics_visible() {
        let prefix = b"connection progress\n";
        let input = Cursor::new(prefix).chain(FailedRead);
        let mut displayed = Vec::new();

        let error = stream_output(input, &mut displayed).expect_err("broken output pipe");

        assert_eq!(displayed, prefix);
        assert!(error.contains("read Pi MCP health output"), "{error}");
        assert!(error.contains("pipe read failed"), "{error}");
    }

    enum DisplayFailure {
        Write,
        Flush,
    }

    struct FailedDisplay(DisplayFailure);

    impl Write for FailedDisplay {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            match self.0 {
                DisplayFailure::Write => Err(io::Error::other("terminal write failed")),
                DisplayFailure::Flush => Ok(buffer.len()),
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            match self.0 {
                DisplayFailure::Write => Ok(()),
                DisplayFailure::Flush => Err(io::Error::other("terminal flush failed")),
            }
        }
    }

    #[test]
    fn display_failures_cannot_report_a_successful_health_check() {
        for (failure, operation) in [
            (DisplayFailure::Write, "display"),
            (DisplayFailure::Flush, "flush"),
        ] {
            let error = stream_output(Cursor::new(b"live output"), FailedDisplay(failure))
                .expect_err("failed live diagnostics display");

            assert!(
                error.contains(&format!("{operation} Pi MCP health output")),
                "{error}"
            );
            assert!(error.contains("terminal"), "{error}");
        }
    }
}
