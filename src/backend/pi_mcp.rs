//! User-scope provisioning through pi-mcp-adapter's documented configuration.

mod actions;
mod health;

use std::collections::BTreeMap;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::{Invocation, McpDecl, McpInventory, McpRemoval, McpStatus};
use actions::{RunMcpCheck, WriteAdapterConfig};

pub(super) fn mcp_list(context: &Invocation) -> Result<McpInventory, String> {
    let configuration = Configuration::load(context)?;
    let (inventory, definitions) = configuration.inventory();
    for (name, definition) in definitions {
        if definition.get("disabled") == Some(&Value::Bool(true)) {
            eprintln!(
                "warning: Pi MCP server '{name}' is registered but disabled in its effective configuration"
            );
        }
    }
    Ok(inventory)
}

pub(super) fn mcp_add(entry: &McpDecl, context: &Invocation) -> Result<(), String> {
    let mut configuration = Configuration::load(context)?;
    if configuration.inventory().0.contains_key(&entry.name) {
        return Ok(());
    }

    let document = configuration.user_document();
    let mut updated = document.body.clone();
    add_entry(&mut updated, entry)?;

    configuration.publish_user(updated)
}

pub(super) fn mcp_remove(name: &str, context: &Invocation) -> Result<McpRemoval, String> {
    let mut configuration = Configuration::load(context)?;
    let document = configuration.user_document();
    let mut updated = document.body.clone();
    let removed = updated
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .is_some_and(|servers| servers.remove(name).is_some());

    if removed {
        configuration.publish_user(updated)?;
    }

    match configuration.inventory().0.remove(name) {
        Some(source) => Ok(McpRemoval::StillRegistered { source }),
        None => Ok(McpRemoval::Removed),
    }
}

pub(super) fn mcp_check(names: &[String], context: &Invocation) -> Result<Vec<McpStatus>, String> {
    if names.is_empty() {
        return Ok(Vec::new());
    }
    let prompt = health::prompt(names)?;
    let output = RunMcpCheck { prompt }.run(context)?;
    health::parse_statuses(&output, names)
}

struct Document {
    path: PathBuf,
    original: Option<String>,
    body: Map<String, Value>,
}

impl Document {
    fn read(path: PathBuf) -> Result<Self, String> {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let body = parse_document(&content, &path.display().to_string())?;
                Ok(Self {
                    path,
                    original: Some(content),
                    body,
                })
            }
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(Self {
                path,
                original: None,
                body: Map::new(),
            }),
            Err(error) => Err(format!("read {}: {error}", path.display())),
        }
    }
}

struct Configuration {
    documents: Vec<Document>,
    user_index: usize,
}

impl Configuration {
    fn load(context: &Invocation) -> Result<Self, String> {
        let home = context.home_dir()?;
        validate_environment(context)?;
        let agent = super::pi::agent_dir(context).map_err(|error| error.to_string())?;
        let user_path = selected_config(context, &agent)?;
        reject_project_packages(context)?;
        reject_package_servers(context)?;

        let global_paths = [
            home.join(".config/mcp/mcp.json"),
            home.join(".agents/mcp.json"),
            home.join(".agents/mcp/mcp.json"),
        ];
        let mut documents = Vec::new();
        for path in global_paths {
            if path != user_path {
                documents.push(Document::read(path)?);
            }
        }
        let user_index = documents.len();
        documents.push(Document::read(user_path.clone())?);
        let ancestors = ancestor_documents(&documents, &home, &context.project_dir)?;
        documents.extend(ancestors);
        for path in [
            context.project_dir.join(".mcp.json"),
            context.project_dir.join(".pi/mcp-adapter.json"),
        ] {
            if path != user_path {
                documents.push(Document::read(path)?);
            }
        }
        for document in &documents {
            validate_sources(document)?;
        }

        Ok(Self {
            documents,
            user_index,
        })
    }

    fn user_document(&self) -> &Document {
        &self.documents[self.user_index]
    }

    fn publish_user(&mut self, body: Map<String, Value>) -> Result<(), String> {
        WriteAdapterConfig { body }.run(self.user_document())?;
        // Different layers may alias the same user-owned file. Re-read every
        // source after publication so removal cannot report a stale alias.
        self.documents = self
            .documents
            .iter()
            .map(|document| Document::read(document.path.clone()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(())
    }

    fn inventory(&self) -> (McpInventory, BTreeMap<String, Map<String, Value>>) {
        let mut sources = McpInventory::new();
        let mut definitions = BTreeMap::<String, Map<String, Value>>::new();
        for document in &self.documents {
            let Some(servers) = document.body.get("mcpServers").and_then(Value::as_object) else {
                continue;
            };
            for (name, definition) in servers {
                let fields = definition.as_object().expect("validated server object");
                definitions
                    .entry(name.clone())
                    .or_default()
                    .extend(fields.clone());
                sources.insert(name.clone(), document.path.clone());
            }
        }
        (sources, definitions)
    }
}

fn ancestor_documents(
    globals: &[Document],
    home: &Path,
    project: &Path,
) -> Result<Vec<Document>, String> {
    let project_paths = [
        project.join(".mcp.json"),
        project.join(".pi/mcp-adapter.json"),
    ];
    let reserved = globals
        .iter()
        .map(|document| document.path.as_path())
        .chain(project_paths.iter().map(PathBuf::as_path))
        .map(path_identity)
        .collect::<Result<Vec<_>, _>>()?;
    let mut selected: Vec<(PathBuf, PathBuf)> = Vec::new();
    for path in ancestor_paths(globals, home, project)? {
        let identity = path_identity(&path)?;
        if reserved.contains(&identity) {
            continue;
        }
        selected.retain(|(existing, _)| existing != &identity);
        selected.push((identity, path));
    }
    selected
        .into_iter()
        .map(|(_, path)| Document::read(path))
        .collect()
}

fn ancestor_paths(
    globals: &[Document],
    home: &Path,
    project: &Path,
) -> Result<Vec<PathBuf>, String> {
    let configured = globals
        .iter()
        .filter_map(|document| {
            document
                .body
                .get("settings")
                .and_then(|settings| settings.get("ancestorConfigRoots"))
        })
        .next_back();
    let Some(configured) = configured else {
        return Ok(Vec::new());
    };
    let roots = configured
        .as_array()
        .ok_or("Pi ancestorConfigRoots must be an array")?;
    if roots.is_empty() {
        return Ok(Vec::new());
    }
    let home = std::fs::canonicalize(home)
        .map_err(|error| format!("resolve home for Pi ancestor discovery: {error}"))?;
    let project = std::fs::canonicalize(project)
        .map_err(|error| format!("resolve project for Pi ancestor discovery: {error}"))?;
    let mut selected = PathBuf::new();
    for root in roots {
        let root = root
            .as_str()
            .ok_or("Pi ancestorConfigRoots entries must be paths")?;
        if !Path::new(root).is_absolute() && !root.starts_with("~/") {
            return Err(
                "Pi ancestorConfigRoots entries must be absolute paths or start with ~/".into(),
            );
        }
        let root = resolve_path(root, &home, &project);
        let root = std::fs::canonicalize(&root)
            .map_err(|error| format!("resolve Pi ancestor root {}: {error}", root.display()))?;
        if !root.is_dir() || !root.starts_with(&home) {
            return Err(format!(
                "Pi ancestor root {} must be a directory under HOME",
                root.display()
            ));
        }
        if project.starts_with(&root) && root.components().count() > selected.components().count() {
            selected = root;
        }
    }
    if selected.as_os_str().is_empty() || project == selected {
        return Ok(Vec::new());
    }

    let directories: Vec<_> = project
        .ancestors()
        .skip(1)
        .take_while(|directory| directory.starts_with(&selected))
        .collect();
    Ok(directories
        .into_iter()
        .rev()
        .flat_map(|directory| {
            [
                directory.join(".mcp.json"),
                directory.join(".pi/mcp-adapter.json"),
            ]
        })
        .collect())
}

fn path_identity(path: &Path) -> Result<PathBuf, String> {
    match std::fs::canonicalize(path) {
        Ok(path) => Ok(path),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(error) => Err(format!(
            "resolve MCP configuration {}: {error}",
            path.display()
        )),
    }
}

fn validate_environment(context: &Invocation) -> Result<(), String> {
    if context.env_value("PI_PACKAGE_DIR").is_some() {
        return Err("Pi MCP inventory does not support PI_PACKAGE_DIR rebranding; use the standard Pi distribution".into());
    }
    if context
        .env_value("PI_MCP_CONFIG_MODE")
        .is_some_and(|value| !value.is_empty())
    {
        return Err("Pi MCP inventory does not support PI_MCP_CONFIG_MODE; remove this undocumented configuration override".into());
    }

    Ok(())
}

fn selected_config(context: &Invocation, agent: &Path) -> Result<PathBuf, String> {
    let options = super::pi::launch_options(context).map_err(|error| error.to_string())?;
    Ok(options
        .mcp_config
        .unwrap_or_else(|| agent.join("mcp-adapter.json")))
}

fn resolve_path(value: &str, home: &Path, project: &Path) -> PathBuf {
    if value == "~" {
        return home.to_path_buf();
    }
    if let Some(relative) = value.strip_prefix("~/") {
        return home.join(relative);
    }
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        project.join(path)
    }
}

fn reject_package_servers(context: &Invocation) -> Result<(), String> {
    let paths = super::pi::installed_package_paths(context).map_err(|error| error.to_string())?;
    for path in paths {
        let manifest = path.join("package.json");
        let content = match std::fs::read_to_string(&manifest) {
            Ok(content) => content,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("read {}: {error}", manifest.display())),
        };
        let value: Value = serde_json::from_str(&content)
            .map_err(|error| format!("parse {}: {error}", manifest.display()))?;
        if value.get("pi").and_then(|pi| pi.get("mcp")).is_some() {
            return Err(format!(
                "Pi MCP inventory does not yet support package-declared pi.mcp servers in {}; manage these servers in /mcp-adapter",
                manifest.display()
            ));
        }
    }
    Ok(())
}

fn reject_project_packages(context: &Invocation) -> Result<(), String> {
    let path = context.project_dir.join(".pi/settings.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("read {}: {error}", path.display())),
    };
    let settings: Value = serde_json::from_str(&content)
        .map_err(|error| format!("parse {}: {error}", path.display()))?;
    let settings = settings
        .as_object()
        .ok_or_else(|| format!("{}: Pi settings must be an object", path.display()))?;
    reject_nonempty_array(
        settings.get("packages"),
        "project package MCP declarations",
        &path,
    )
}

fn parse_document(content: &str, label: &str) -> Result<Map<String, Value>, String> {
    let root: Value = serde_json::from_str(content).map_err(|error| {
        format!("parse {label}: {error}; ACE requires strict JSON and does not rewrite JSONC")
    })?;
    let Value::Object(document) = root else {
        return Err(format!("{label}: MCP configuration must be an object"));
    };
    if let Some(servers) = document.get("mcpServers") {
        let servers = servers
            .as_object()
            .ok_or_else(|| format!("{label}: mcpServers must be an object"))?;
        for server in servers.values() {
            validate_server(server, label)?;
        }
    }
    if document.contains_key("mcp-servers") {
        return Err(format!(
            "{label}: legacy mcp-servers is unsupported; migrate it to documented mcpServers before using ACE"
        ));
    }
    Ok(document)
}

fn validate_server(value: &Value, label: &str) -> Result<(), String> {
    let server = value
        .as_object()
        .ok_or_else(|| format!("{label}: every MCP server definition must be an object"))?;
    for field in ["url", "command", "cwd", "socket"] {
        if server.get(field).is_some_and(|value| !value.is_string()) {
            return Err(format!("{label}: MCP server {field} must be a string"));
        }
    }
    if server
        .get("disabled")
        .is_some_and(|value| !value.is_boolean())
    {
        return Err(format!("{label}: MCP server disabled must be a boolean"));
    }
    for field in ["headers", "env"] {
        let Some(values) = server.get(field) else {
            continue;
        };
        let values = values
            .as_object()
            .ok_or_else(|| format!("{label}: MCP server {field} must be an object"))?;
        if values.values().any(|value| !value.is_string()) {
            return Err(format!(
                "{label}: MCP server {field} values must be strings"
            ));
        }
    }
    if let Some(args) = server.get("args") {
        let args = args
            .as_array()
            .ok_or_else(|| format!("{label}: MCP server args must be an array"))?;
        if args.iter().any(|value| !value.is_string()) {
            return Err(format!("{label}: MCP server args must contain strings"));
        }
    }
    Ok(())
}

fn validate_sources(document: &Document) -> Result<(), String> {
    for key in ["imports", "claudePlugins"] {
        reject_nonempty_array(document.body.get(key), key, &document.path)?;
    }
    let Some(settings) = document.body.get("settings") else {
        return Ok(());
    };
    let settings = settings
        .as_object()
        .ok_or_else(|| format!("{}: settings must be an object", document.path.display()))?;
    reject_nonempty_array(
        settings.get("agentPluginPaths"),
        "agentPluginPaths",
        &document.path,
    )?;
    match settings.get("hostConfigDiscovery") {
        None | Some(Value::String(_))
            if settings
                .get("hostConfigDiscovery")
                .is_none_or(|value| value == "off" || value == "prompt") =>
        {
            Ok(())
        }
        _ => Err(format!(
            "{}: ACE cannot inventory hostConfigDiscovery; use off or manage MCP through /mcp-adapter",
            document.path.display()
        )),
    }
}

fn reject_nonempty_array(value: Option<&Value>, field: &str, path: &Path) -> Result<(), String> {
    match value {
        None => Ok(()),
        Some(Value::Array(values)) if values.is_empty() => Ok(()),
        _ => Err(format!(
            "{}: ACE cannot inventory {field}; manage those MCP sources through /mcp-adapter",
            path.display()
        )),
    }
}

fn add_entry(document: &mut Map<String, Value>, entry: &McpDecl) -> Result<(), String> {
    let mut headers = Map::new();
    for (name, value) in &entry.headers {
        if value.contains("${") || value.contains("$env:") || value.contains("{env:") {
            return Err(format!(
                "Pi cannot preserve literal header '{name}' containing adapter environment interpolation syntax"
            ));
        }
        let value = if value.starts_with('!') {
            format!("!{value}")
        } else {
            value.clone()
        };
        headers.insert(name.clone(), Value::String(value));
    }
    if entry.url.contains("${") || entry.url.contains("$env:") {
        return Err("Pi cannot preserve a literal MCP URL containing adapter environment interpolation syntax".into());
    }

    let servers = document
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(Map::new()));
    let servers = servers
        .as_object_mut()
        .ok_or("mcpServers must be an object")?;
    let mut definition = Map::from_iter([("url".into(), Value::String(entry.url.clone()))]);
    if !headers.is_empty() {
        definition.insert("headers".into(), Value::Object(headers));
    }
    servers
        .entry(entry.name.clone())
        .or_insert(Value::Object(definition));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn malformed_documents_are_not_empty_inventory() {
        for content in [
            "",
            "broken",
            "[]",
            r#"{"mcpServers": []}"#,
            r#"{"mcpServers":{"a":false}}"#,
        ] {
            assert!(
                parse_document(content, "fixture").is_err(),
                "accepted {content}"
            );
        }
    }

    #[test]
    fn malformed_server_fields_are_rejected_before_registration() {
        for content in [
            r#"{"mcpServers":{"sample":{"url":7}}}"#,
            r#"{"mcpServers":{"sample":{"headers":{"Authorization":false}}}}"#,
            r#"{"mcpServers":{"sample":{"disabled":"true"}}}"#,
        ] {
            assert!(
                parse_document(content, "fixture").is_err(),
                "accepted {content}"
            );
        }
    }

    #[test]
    fn registration_preserves_existing_definitions_and_unrelated_values() {
        let existing = serde_json::json!({
            "settings": {"directTools": "search"},
            "extension": [1, 2],
            "mcpServers": {"existing": {"url": "https://existing", "auth": "oauth"}}
        });
        let mut document = existing.as_object().expect("object").clone();
        let entry = super::super::McpDecl {
            name: "new".into(),
            url: "https://new".into(),
            headers: HashMap::from([("Authorization".into(), "!literal-secret".into())]),
            instructions: String::new(),
        };

        add_entry(&mut document, &entry).expect("add");

        assert_eq!(document["settings"], existing["settings"]);
        assert_eq!(document["extension"], existing["extension"]);
        assert_eq!(
            document["mcpServers"]["existing"],
            existing["mcpServers"]["existing"]
        );
        assert_eq!(
            document["mcpServers"]["new"]["headers"]["Authorization"],
            "!!literal-secret"
        );
    }

    #[test]
    fn registration_does_not_overwrite_an_existing_name() {
        let mut document = serde_json::json!({"mcpServers":{"existing":{"url":"https://old"}}})
            .as_object()
            .expect("object")
            .clone();
        let original = document.clone();
        let entry = super::super::McpDecl {
            name: "existing".into(),
            url: "https://replacement".into(),
            headers: HashMap::new(),
            instructions: String::new(),
        };

        add_entry(&mut document, &entry).expect("existing name is a no-op");

        assert_eq!(document, original);
    }
}
