use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Command, ExitStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeMode {
    Fresh,
    Latest,
}

pub struct SessionProcess {
    program: Box<str>,
    args: Box<[String]>,
    env: HashMap<String, String>,
    working_dir: PathBuf,
}

impl SessionProcess {
    pub fn new(
        program: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            program: program.into_boxed_str(),
            args: args.into_boxed_slice(),
            env,
            working_dir,
        }
    }

    pub fn run(self) -> std::io::Result<()> {
        let status = self.wait()?;
        crate::platform::propagate_exit_status(status);
        Ok(())
    }

    fn command(&self) -> Command {
        let mut command = Command::new(self.program.as_ref());
        command.args(&self.args);
        command.envs(&self.env);
        command.current_dir(&self.working_dir);
        command
    }

    fn wait(self) -> std::io::Result<ExitStatus> {
        let _supervision = crate::platform::begin_child_supervision();
        let mut child = self.command().spawn()?;
        child.wait()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn process_applies_its_command_contract() {
        let process = SessionProcess::new(
            "wrapper".to_string(),
            vec!["backend".to_string(), "--flag".to_string()],
            HashMap::from([("TOKEN".to_string(), "secret".to_string())]),
            PathBuf::from("/project"),
        );

        let command = process.command();

        assert_eq!(command.get_program(), "wrapper");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["backend", "--flag"]
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("/project")));
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "TOKEN")
                .and_then(|(_, value)| value),
            Some(std::ffi::OsStr::new("secret")),
        );
    }

    #[cfg(unix)]
    #[test]
    fn process_returns_the_child_status() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let process = SessionProcess::new(
            "sh".to_string(),
            vec!["-c".to_string(), "exit 7".to_string()],
            HashMap::new(),
            temp.path().to_path_buf(),
        );

        let status = process.wait().expect("run supervised session");

        assert_eq!(status.code(), Some(7));
    }

    #[cfg(unix)]
    #[test]
    fn process_applies_environment_and_working_directory() {
        let temp = tempfile::tempdir().expect("create temp dir");
        std::fs::write(temp.path().join("marker"), "present").expect("write marker");
        let process = SessionProcess::new(
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "test \"$TOKEN\" = secret && test -f marker".to_string(),
            ],
            HashMap::from([("TOKEN".to_string(), "secret".to_string())]),
            temp.path().to_path_buf(),
        );

        let status = process.wait().expect("run supervised session");

        assert!(status.success());
    }
}
