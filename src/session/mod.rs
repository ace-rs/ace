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
// The runtime allocator lands with the graph executor; backend materializers consume
// these endpoint values now so their process topology is concrete and testable first.
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
pub enum GraphError {
    #[error("a component graph must contain at least one node")]
    Empty,
    #[error("component role `{0:?}` occurs more than once")]
    DuplicateRole(Role),
    #[error("component `{role:?}` depends on missing component `{dependency:?}`")]
    MissingDependency { role: Role, dependency: Role },
    #[error("component graph contains a dependency cycle")]
    Cycle,
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
                "server components require a graph executor",
            ),
            Role::Session => crate::platform::exec_replace(self.command()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    component: Component,
    dependencies: Vec<Role>,
}

impl Node {
    pub fn new(component: Component, dependencies: Vec<Role>) -> Self {
        Self {
            component,
            dependencies,
        }
    }

    #[cfg(test)]
    pub fn component(&self) -> &Component {
        &self.component
    }

    #[cfg(test)]
    pub fn dependencies(&self) -> &[Role] {
        &self.dependencies
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    nodes: Vec<Node>,
}

impl Graph {
    pub fn try_new(nodes: Vec<Node>) -> Result<Self, GraphError> {
        if nodes.is_empty() {
            return Err(GraphError::Empty);
        }

        let mut roles = HashSet::with_capacity(nodes.len());
        for node in &nodes {
            let role = node.component.role();
            if !roles.insert(role) {
                return Err(GraphError::DuplicateRole(role));
            }
        }
        for node in &nodes {
            let role = node.component.role();
            for dependency in &node.dependencies {
                if !roles.contains(dependency) {
                    return Err(GraphError::MissingDependency {
                        role,
                        dependency: *dependency,
                    });
                }
            }
        }

        let mut remaining = nodes;
        let mut ordered = Vec::with_capacity(remaining.len());
        let mut satisfied = HashSet::with_capacity(remaining.len());
        while !remaining.is_empty() {
            let Some(index) = remaining.iter().position(|node| {
                node.dependencies
                    .iter()
                    .all(|role| satisfied.contains(role))
            }) else {
                return Err(GraphError::Cycle);
            };
            let node = remaining.remove(index);
            satisfied.insert(node.component.role());
            ordered.push(node);
        }

        Ok(Self { nodes: ordered })
    }

    #[cfg(test)]
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn exec_replace(self) -> std::io::Error {
        let mut nodes = self.nodes.into_iter();
        let Some(node) = nodes.next() else {
            return std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "cannot execute an empty component graph",
            );
        };
        if nodes.next().is_some() {
            return std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "multi-component graphs require a graph executor",
            );
        }
        node.component.exec_replace()
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
    fn graph_orders_dependencies_before_dependants() {
        let graph = Graph::try_new(vec![
            Node::new(component(Role::Session), vec![Role::Server]),
            Node::new(component(Role::Server), Vec::new()),
        ])
        .expect("valid graph");

        let roles = graph
            .nodes()
            .iter()
            .map(|node| node.component().role())
            .collect::<Vec<_>>();

        assert_eq!(roles, [Role::Server, Role::Session]);
    }

    #[test]
    fn graph_rejects_empty_input() {
        assert_eq!(Graph::try_new(Vec::new()), Err(GraphError::Empty));
    }

    #[test]
    fn graph_rejects_duplicate_roles() {
        let error = Graph::try_new(vec![
            Node::new(component(Role::Session), Vec::new()),
            Node::new(component(Role::Session), Vec::new()),
        ])
        .expect_err("duplicate role must fail");

        assert_eq!(error, GraphError::DuplicateRole(Role::Session));
    }

    #[test]
    fn graph_rejects_missing_dependencies() {
        let error = Graph::try_new(vec![Node::new(
            component(Role::Session),
            vec![Role::Server],
        )])
        .expect_err("missing dependency must fail");

        assert_eq!(
            error,
            GraphError::MissingDependency {
                role: Role::Session,
                dependency: Role::Server,
            }
        );
    }

    #[test]
    fn graph_rejects_cycles() {
        let error = Graph::try_new(vec![
            Node::new(component(Role::Server), vec![Role::Session]),
            Node::new(component(Role::Session), vec![Role::Server]),
        ])
        .expect_err("cycle must fail");

        assert_eq!(error, GraphError::Cycle);
    }

    #[test]
    fn multi_component_graph_requires_an_executor() {
        let graph = Graph::try_new(vec![
            Node::new(component(Role::Server), Vec::new()),
            Node::new(component(Role::Session), vec![Role::Server]),
        ])
        .expect("valid graph");

        let error = graph.exec_replace();

        assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
        assert_eq!(
            error.to_string(),
            "multi-component graphs require a graph executor"
        );
    }
}
