use std::collections::{HashMap, HashSet};
use std::num::NonZeroU16;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Server,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
// The runtime allocator lands with controlled execution; backend materializers consume
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

    fn spawn(self) -> std::io::Result<std::process::Child> {
        self.command().spawn()
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

    pub fn run(self) -> std::io::Result<()> {
        let status = self.wait()?;
        crate::platform::propagate_exit_status(status);
        Ok(())
    }

    fn wait(mut self) -> std::io::Result<std::process::ExitStatus> {
        if self.items.len() != 1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "multi-component sessions require readiness-aware supervision",
            ));
        }

        let component = self.items.remove(0);
        debug_assert_eq!(component.role(), Role::Session);
        let mut child = component.spawn()?;
        let _supervision = crate::platform::begin_child_supervision();

        thread::scope(|scope| {
            let (sender, receiver) = mpsc::sync_channel(1);
            let waiter = scope.spawn(move || {
                sender.send(child.wait()).map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "component status receiver stopped",
                    )
                })
            });
            let received = receiver
                .recv()
                .map_err(|_| std::io::Error::other("component waiter stopped"));
            waiter
                .join()
                .map_err(|_| std::io::Error::other("component waiter panicked"))??;
            received?
        })
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

    #[cfg(unix)]
    #[test]
    fn supervised_session_returns_the_terminal_status() {
        let temp = tempfile::tempdir().expect("create temp dir");
        let session = Component::new(
            Role::Session,
            "sh".to_string(),
            vec!["-c".to_string(), "exit 7".to_string()],
            HashMap::new(),
            temp.path().to_path_buf(),
        );
        let components = Components::try_new(vec![session]).expect("valid components");

        let status = components.wait().expect("run supervised session");

        assert_eq!(status.code(), Some(7));
    }

    #[cfg(unix)]
    #[test]
    fn supervised_session_applies_process_contract() {
        let temp = tempfile::tempdir().expect("create temp dir");
        std::fs::write(temp.path().join("marker"), "present").expect("write marker");
        let session = Component::new(
            Role::Session,
            "sh".to_string(),
            vec![
                "-c".to_string(),
                "test \"$TOKEN\" = secret && test -f marker".to_string(),
            ],
            HashMap::from([("TOKEN".to_string(), "secret".to_string())]),
            temp.path().to_path_buf(),
        );
        let components = Components::try_new(vec![session]).expect("valid components");

        let status = components.wait().expect("run supervised session");

        assert!(status.success());
    }

    #[test]
    fn multi_component_session_requires_readiness_aware_supervision() {
        let components =
            Components::try_new(vec![component(Role::Server), component(Role::Session)])
                .expect("valid components");

        let error = components.run().expect_err("multi-component run must fail");

        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        assert_eq!(
            error.to_string(),
            "multi-component sessions require readiness-aware supervision"
        );
    }
}
