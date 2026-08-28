use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    role: Role,
    program: String,
    args: Vec<String>,
    env: HashMap<String, String>,
    working_dir: PathBuf,
}

impl Component {
    pub fn new(
        role: Role,
        program: String,
        args: Vec<String>,
        env: HashMap<String, String>,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            role,
            program,
            args,
            env,
            working_dir,
        }
    }

    #[cfg(test)]
    pub fn role(&self) -> Role {
        self.role
    }

    #[cfg(test)]
    pub fn program(&self) -> &str {
        &self.program
    }

    #[cfg(test)]
    pub fn args(&self) -> &[String] {
        &self.args
    }

    #[cfg(test)]
    pub fn env(&self) -> &HashMap<String, String> {
        &self.env
    }

    #[cfg(test)]
    pub fn working_dir(&self) -> &std::path::Path {
        &self.working_dir
    }

    fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command.args(&self.args);
        command.envs(&self.env);
        command.current_dir(&self.working_dir);
        command
    }

    pub fn exec_replace(self) -> std::io::Error {
        match self.role {
            Role::Session => crate::platform::exec_replace(self.command()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn component_carries_process_contract() {
        let component = Component::new(
            Role::Session,
            "wrapper".to_string(),
            vec!["backend".to_string(), "--flag".to_string()],
            HashMap::from([("TOKEN".to_string(), "secret".to_string())]),
            PathBuf::from("/project"),
        );

        let command = component.command();

        assert_eq!(component.role(), Role::Session);
        assert_eq!(component.args(), ["backend", "--flag"]);
        assert_eq!(
            component.env().get("TOKEN").map(String::as_str),
            Some("secret")
        );
        assert_eq!(component.working_dir(), Path::new("/project"));
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
}
