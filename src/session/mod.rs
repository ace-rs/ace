use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Server,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
// The runtime allocator lands with the component executor; backend materializers consume
// these endpoint values now so their process sequence is concrete and testable first.
#[allow(dead_code)]
pub enum ControlEndpoint {
    Unix(PathBuf),
    LoopbackHttp(NonZeroU16),
}

impl ControlEndpoint {
    pub fn unix_url(&self) -> Option<String> {
        match self {
            Self::Unix(path) => Some(format!("unix://{}", path.display())),
            Self::LoopbackHttp(_) => None,
        }
    }

    pub fn loopback_http(&self) -> Option<(&'static str, NonZeroU16, String)> {
        match self {
            Self::Unix(_) => None,
            Self::LoopbackHttp(port) => {
                Some(("127.0.0.1", *port, format!("http://127.0.0.1:{port}")))
            }
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ComponentsError {
    #[error("a session component list must contain at least one component")]
    Empty,
    #[error("component role `{0:?}` occurs more than once")]
    DuplicateRole(Role),
    #[error("a session component list must contain a session component")]
    MissingSession,
    #[error("the session component must be last")]
    SessionNotTerminal,
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
    #[cfg(test)]
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

    pub fn from_launch(
        role: Role,
        launch: &[String],
        fallback_program: &str,
        backend_args: Vec<String>,
        env: &HashMap<String, String>,
        working_dir: &Path,
    ) -> Self {
        let (program, prefix) = launch
            .split_first()
            .map(|(program, rest)| (program.as_str(), rest))
            .unwrap_or((fallback_program, &[][..]));
        let mut args = prefix.to_vec();
        args.extend(backend_args);

        Self {
            role,
            program: program.to_string(),
            args,
            env: env.clone(),
            working_dir: working_dir.to_path_buf(),
        }
    }

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
            Role::Server => std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "server components require a component executor",
            ),
            Role::Session => crate::platform::exec_replace(self.command()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Components {
    items: Vec<Component>,
}

impl Components {
    pub fn try_new(items: Vec<Component>) -> Result<Self, ComponentsError> {
        if items.is_empty() {
            return Err(ComponentsError::Empty);
        }

        let mut roles = HashSet::with_capacity(items.len());
        for component in &items {
            let role = component.role();
            if !roles.insert(role) {
                return Err(ComponentsError::DuplicateRole(role));
            }
        }
        let Some(session_index) = items
            .iter()
            .position(|component| component.role() == Role::Session)
        else {
            return Err(ComponentsError::MissingSession);
        };
        if session_index != items.len() - 1 {
            return Err(ComponentsError::SessionNotTerminal);
        }

        Ok(Self { items })
    }

    #[cfg(test)]
    pub fn items(&self) -> &[Component] {
        &self.items
    }

    pub fn exec_replace(mut self) -> std::io::Error {
        if self.items.len() != 1 {
            return std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "multi-component sessions require a component executor",
            );
        }

        self.items.remove(0).exec_replace()
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

    fn component(role: Role) -> Component {
        Component::new(
            role,
            "backend".to_string(),
            Vec::new(),
            HashMap::new(),
            PathBuf::from("/project"),
        )
    }

    #[test]
    fn components_require_a_terminal_session() {
        assert_eq!(Components::try_new(Vec::new()), Err(ComponentsError::Empty));

        let duplicate = Components::try_new(vec![
            component(Role::Server),
            component(Role::Server),
            component(Role::Session),
        ])
        .expect_err("duplicate roles must fail");
        assert_eq!(duplicate, ComponentsError::DuplicateRole(Role::Server));

        let missing = Components::try_new(vec![component(Role::Server)])
            .expect_err("a session component is required");
        assert_eq!(missing, ComponentsError::MissingSession);

        let nonterminal =
            Components::try_new(vec![component(Role::Session), component(Role::Server)])
                .expect_err("the session component must be terminal");
        assert_eq!(nonterminal, ComponentsError::SessionNotTerminal);
    }

    #[test]
    fn components_preserve_startup_order() {
        let components =
            Components::try_new(vec![component(Role::Server), component(Role::Session)])
                .expect("valid components");
        let roles = components
            .items()
            .iter()
            .map(Component::role)
            .collect::<Vec<_>>();

        assert_eq!(roles, [Role::Server, Role::Session]);
    }

    #[test]
    fn components_accept_a_single_terminal_session() {
        let components = Components::try_new(vec![component(Role::Session)])
            .expect("a normal session is a complete component list");

        assert_eq!(components.items()[0].role(), Role::Session);
    }

    #[test]
    fn multi_component_session_requires_an_executor() {
        let components =
            Components::try_new(vec![component(Role::Server), component(Role::Session)])
                .expect("valid components");

        let error = components.exec_replace();

        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        assert_eq!(
            error.to_string(),
            "multi-component sessions require a component executor"
        );
    }
}
