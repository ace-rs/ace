#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use assert_cmd::Command;

// -- Flaude record parsing --

#[derive(Debug)]
pub struct FlaudeRecord {
    pub action: String,

    // exec_session
    pub trust: String,
    pub resume: bool,
    pub backend_mode: String,
    pub session_prompt: String,

    // exec (shared)
    pub env: HashMap<String, String>,
    pub extra_args: Vec<String>,
    pub cmd: Vec<String>,

    // exec_one_shot
    pub prompt_kind: Option<String>,
    pub prompt_text: Option<String>,

    // mcp
    pub name: String,
    pub url: String,
    pub headers: Vec<String>,
}

fn json_str_vec(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn json_str_map(v: &serde_json::Value) -> HashMap<String, String> {
    v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

fn parse_flaude_records(path: &Path) -> Vec<FlaudeRecord> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .filter(|l| !l.is_empty())
        .map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).expect("parse flaude record");
            FlaudeRecord {
                action: v["action"].as_str().unwrap_or_default().to_string(),

                trust: v["trust"].as_str().unwrap_or_default().to_string(),
                resume: v["resume"].as_bool().unwrap_or_default(),
                backend_mode: v["backend_mode"].as_str().unwrap_or_default().to_string(),
                session_prompt: v["session_prompt"].as_str().unwrap_or_default().to_string(),

                env: json_str_map(&v["env"]),
                extra_args: json_str_vec(&v["extra_args"]),
                cmd: json_str_vec(&v["cmd"]),

                prompt_kind: v["prompt"]["kind"].as_str().map(String::from),
                prompt_text: v["prompt"]["text"].as_str().map(String::from),

                name: v["name"].as_str().unwrap_or_default().to_string(),
                url: v["url"].as_str().unwrap_or_default().to_string(),
                headers: json_str_vec(&v["headers"]),
            }
        })
        .collect()
}

/// A fake "remote" school: bare origin repo + cache clone at the XDG path.
/// Use `git_in(&self.cache, ...)` or `git_in(&self.origin, ...)` to manipulate.
pub struct RemoteSchool {
    pub origin: PathBuf,
    pub cache: PathBuf,
}

pub struct TestEnv {
    _tmp: tempfile::TempDir,
    root: PathBuf,
}

impl TestEnv {
    pub fn new() -> Self {
        let tmp = tempfile::TempDir::new().expect("create temp dir");
        // Canonicalize to resolve macOS /var → /private/var symlinks.
        let root = tmp.path().canonicalize().expect("canonicalize temp dir");
        Self { _tmp: tmp, root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        assert!(
            !Path::new(rel).is_absolute(),
            "TestEnv::path() rejects absolute paths: {rel}"
        );
        self.root.join(rel)
    }

    /// Set up the workdir as a school-repo that dogfoods itself: `school.toml`
    /// with the given contents, plus an `ace.toml` with `school = "."` so
    /// `Ace::require_linked_school` resolves to the workdir via the specifier.
    pub fn write_dogfood_school(&self, school_toml: &str) {
        self.write_file("school.toml", school_toml);
        if !self.path("ace.toml").exists() {
            self.write_file("ace.toml", "school = \".\"\n");
        }
    }

    pub fn write_file(&self, rel: &str, contents: &str) {
        let path = self.path(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent dirs");
        }
        std::fs::write(&path, contents).expect("write file");
    }

    pub fn write_executable(&self, rel: &str, contents: &str) {
        use std::os::unix::fs::PermissionsExt;

        self.write_file(rel, contents);

        let path = self.path(rel);
        let mut perms = std::fs::metadata(&path)
            .expect("stat executable")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("chmod executable");
    }

    pub fn read_file(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path(rel)).expect("read file")
    }

    pub fn mkdir(&self, rel: &str) {
        std::fs::create_dir_all(self.path(rel)).expect("mkdir");
    }

    pub fn symlink(&self, target: &str, link: &str) {
        let target_path = self.path(target);
        let link_path = self.path(link);
        if let Some(parent) = link_path.parent() {
            std::fs::create_dir_all(parent).expect("create link parent dirs");
        }
        std::os::unix::fs::symlink(&target_path, &link_path).expect("create symlink");
    }

    pub fn git_init(&self) {
        self.git_init_in(&self.root);
    }

    /// `git init` a subdirectory of the sandbox — for fixtures that need a repo
    /// somewhere other than the project root, e.g. a legacy cache clone.
    pub fn git_init_at(&self, rel: &str) {
        self.git_init_in(&self.path(rel));
    }

    fn git_init_in(&self, dir: &Path) {
        let status = std::process::Command::new("git")
            .args(["init", "--quiet", "--template="])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(dir)
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");
    }

    pub fn assert_exists(&self, rel: &str) {
        let path = self.path(rel);
        assert!(path.exists(), "{} should exist", path.display());
    }

    pub fn assert_not_exists(&self, rel: &str) {
        let path = self.path(rel);
        assert!(!path.exists(), "{} should not exist", path.display());
    }

    /// Assert that the given path is a real directory, not a symlink. Useful
    /// for the new per-skill layout where `<backend>/skills/` is a real dir.
    pub fn assert_skills_dir_is_real(&self, rel: &str) {
        let path = self.path(rel);
        let meta = path
            .symlink_metadata()
            .unwrap_or_else(|_| panic!("{} should exist", path.display()));
        assert!(
            !meta.file_type().is_symlink(),
            "{} should be a real dir, not a symlink",
            path.display()
        );
        assert!(meta.is_dir(), "{} should be a directory", path.display());
    }

    pub fn assert_symlink(&self, link: &str, expected_target: &str) {
        let link_path = self.path(link);
        let meta = link_path
            .symlink_metadata()
            .unwrap_or_else(|_| panic!("{} should exist", link_path.display()));
        assert!(
            meta.file_type().is_symlink(),
            "{} should be a symlink",
            link_path.display()
        );

        let actual = std::fs::read_link(&link_path)
            .unwrap_or_else(|_| panic!("read_link {}", link_path.display()));
        let expected = self.path(expected_target);
        assert_eq!(actual, expected, "symlink target mismatch");
    }

    pub fn assert_contains(&self, rel: &str, needle: &str) {
        let content = self.read_file(rel);
        assert!(
            content.contains(needle),
            "{rel} should contain {needle:?}, got:\n{content}"
        );
    }

    pub fn assert_not_contains(&self, rel: &str, needle: &str) {
        let content = self.read_file(rel);
        assert!(
            !content.contains(needle),
            "{rel} should NOT contain {needle:?}, got:\n{content}"
        );
    }

    pub fn git_commit(&self, message: &str) {
        let status = std::process::Command::new("git")
            .args(["add", "-A"])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(&self.root)
            .status()
            .expect("git add");
        assert!(status.success(), "git add failed");

        let status = std::process::Command::new("git")
            .args([
                "-c",
                "user.email=test@test.com",
                "-c",
                "user.name=Test",
                "commit",
                "-m",
                message,
                "--allow-empty",
            ])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(&self.root)
            .status()
            .expect("git commit");
        assert!(status.success(), "git commit failed");
    }

    /// Create a minimal embedded school: school.toml + one skill.
    pub fn setup_embedded_school(&self, name: &str) {
        self.write_file("school.toml", &format!("name = \"{name}\"\n"));
        self.mkdir("skills/maverick");
        self.write_file("skills/maverick/SKILL.md", "# Maverick\n");
    }

    /// Create an embedded school and run `ace setup .` — the most common test fixture.
    pub fn setup_embedded(&self, name: &str) {
        self.git_init();
        self.setup_embedded_school(name);
        self.ace().args(["setup", "."]).assert().success();
    }

    pub fn git_status(&self) -> String {
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain"])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(&self.root)
            .output()
            .expect("git status");
        assert!(output.status.success(), "git status failed");
        String::from_utf8(output.stdout).expect("git status utf8")
    }

    /// Write the flaude MCP list file (one name per line).
    /// Flaude's `mcp_list()` reads `$HOME/.flaude-mcp-list`.
    pub fn write_flaude_mcp_list(&self, names: &[&str]) {
        self.write_file(".flaude-mcp-list", &names.join("\n"));
    }

    /// Read MCP registration records written by flaude's `mcp_add()`.
    pub fn read_flaude_mcp_records(&self) -> Vec<FlaudeRecord> {
        parse_flaude_records(&self.path(".flaude-mcp-records.jsonl"))
            .into_iter()
            .filter(|r| r.action == "mcp_add")
            .collect()
    }

    /// Read interactive session exec records written by flaude.
    pub fn read_flaude_exec_records(&self) -> Vec<FlaudeRecord> {
        parse_flaude_records(&self.path(".flaude-exec-records.jsonl"))
            .into_iter()
            .filter(|r| r.action == "exec_session")
            .collect()
    }

    /// Read one-shot exec records written by flaude.
    pub fn read_flaude_one_shot_records(&self) -> Vec<FlaudeRecord> {
        parse_flaude_records(&self.path(".flaude-exec-records.jsonl"))
            .into_iter()
            .filter(|r| r.action == "exec_one_shot")
            .collect()
    }

    /// Run a git command in an arbitrary directory. Returns stdout as String.
    pub fn git_in(&self, dir: &Path, args: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .args(args)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .current_dir(dir)
            .output()
            .unwrap_or_else(|e| panic!("git {:?}: {e}", args));
        assert!(
            output.status.success(),
            "git {:?} failed in {}: {}",
            args,
            dir.display(),
            String::from_utf8_lossy(&output.stderr),
        );
        String::from_utf8(output.stdout).expect("git output utf8")
    }

    /// Set up a fake remote school: bare origin, cache clone, index entry, ace.toml.
    /// Project dir gets git init + ace.toml with flaude backend.
    ///
    /// Per-specifier templates are built once per binary; per-test cost is a
    /// pair of `cp -R`s and one `git remote set-url`. Tests are free to push
    /// commits to the returned `origin` (they get a private copy).
    pub fn setup_remote_school(&self, specifier: &str) -> RemoteSchool {
        let tpl = remote_school_template(specifier);

        let origin = self.path("origin.git");
        let cache = self.path(&format!("data/ace/{specifier}"));
        copy_tree(&tpl.origin, &origin);
        std::fs::create_dir_all(cache.parent().expect("cache parent"))
            .expect("create cache parent");
        copy_tree(&tpl.cache, &cache);

        // Rewrite the cache clone's origin URL to point at the per-test bare
        // origin (template copies still reference the template's path).
        self.git_in(
            &cache,
            &[
                "remote",
                "set-url",
                "origin",
                origin.to_str().expect("origin path"),
            ],
        );

        let index_path = self.path("data/ace/index.toml");
        std::fs::create_dir_all(index_path.parent().expect("index parent"))
            .expect("create index parent");
        std::fs::write(
            &index_path,
            format!("[[school]]\nspecifier = \"{specifier}\"\nrepo = \"{specifier}\"\n"),
        )
        .expect("write index.toml");

        // insteadOf redirect so any re-clone (self-heal path) goes through
        // the sandbox origin instead of github.com.
        append_gitconfig_redirect(
            &self.path(".gitconfig"),
            &format!("https://github.com/{specifier}.git"),
            &origin,
        );

        self.git_init();
        self.write_file(
            "ace.toml",
            &format!("school = \"{specifier}\"\nbackend = \"flaude\"\n"),
        );

        RemoteSchool { origin, cache }
    }

    /// Set up a bare origin repo containing skill folders at the given paths
    /// and a gitconfig redirect so `ace import <specifier>` clones from the
    /// sandbox instead of hitting github.com. `skill_paths` are relative to
    /// the repo root — e.g. `"skills/.experimental/shell"`.
    pub fn setup_tiered_origin(&self, specifier: &str, skill_paths: &[&str]) {
        let origin = self.path(&format!("origins/{specifier}.git"));
        let work = self.path(&format!("_origin_work_{}", specifier.replace('/', "_")));

        std::fs::create_dir_all(&origin).expect("create origin dir");
        self.git_in(
            &origin,
            &["init", "--bare", "--quiet", "--template=", "-b", "main"],
        );

        self.git_in(
            self.root(),
            &[
                "clone",
                "--quiet",
                origin.to_str().expect("origin path"),
                work.to_str().expect("work path"),
            ],
        );

        for rel in skill_paths {
            let skill_dir = work.join(rel);
            std::fs::create_dir_all(&skill_dir).expect("create skill dir");
            let name = skill_dir
                .file_name()
                .and_then(|n| n.to_str())
                .expect("skill dir name");
            std::fs::write(skill_dir.join("SKILL.md"), format!("# {name}\n"))
                .expect("write SKILL.md");
        }

        self.git_in(&work, &["add", "-A"]);
        self.git_in(
            &work,
            &[
                "-c",
                "user.email=test@test.com",
                "-c",
                "user.name=Test",
                "commit",
                "-m",
                "seed",
            ],
        );
        self.git_in(&work, &["push", "--quiet"]);
        std::fs::remove_dir_all(&work).expect("remove work dir");

        // gitconfig redirect: https://github.com/<specifier>.git → file://origin
        // Using insteadOf on the full URL avoids interfering with any other
        // GitHub access the test might make.
        let gh_url = format!("https://github.com/{specifier}.git");
        let file_url = format!("file://{}", origin.display());
        let config_block = format!("[url \"{file_url}\"]\n\tinsteadOf = {gh_url}\n");

        let gitconfig_path = self.path(".gitconfig");
        if gitconfig_path.exists() {
            let mut existing = std::fs::read_to_string(&gitconfig_path).expect("read gitconfig");
            existing.push_str(&config_block);
            std::fs::write(&gitconfig_path, existing).expect("append gitconfig");
        } else {
            std::fs::write(&gitconfig_path, config_block).expect("write gitconfig");
        }
    }

    /// Set up an embedded school with flaude backend. Common fixture for
    /// MCP and exec integration tests.
    pub fn setup_flaude_school(&self, school_toml: &str) {
        self.git_init();
        self.write_file("school.toml", school_toml);
        self.write_file("ace.toml", "school = \".\"\nbackend = \"flaude\"\n");
        self.mkdir("skills/test-skill");
        self.write_file("skills/test-skill/SKILL.md", "# Test\n");
        self.write_file("CLAUDE.md", "# Test\n");
        self.mkdir(".claude");
        self.symlink("skills", ".claude/skills");
    }
    /// Returns an `assert_cmd::Command` for the `ace` binary, pre-configured
    /// with a clean environment and sandbox paths.
    pub fn ace(&self) -> Command {
        let mut cmd = Command::from_std(std::process::Command::new(assert_cmd::cargo_bin!("ace")));
        cmd.env_clear();
        cmd.env("HOME", self.root());
        cmd.env("PATH", std::env::var("PATH").unwrap_or_default());
        cmd.env("XDG_CONFIG_HOME", self.path("config"));
        cmd.env("XDG_CACHE_HOME", self.path("cache"));
        cmd.env("XDG_DATA_HOME", self.path("data"));
        cmd.env("GIT_TERMINAL_PROMPT", "0");
        cmd.env("TERM", "dumb");
        cmd.current_dir(self.root());
        cmd
    }

    pub fn ace_with_path_prefix(&self, prefix: &Path) -> Command {
        let mut cmd = self.ace();
        let path = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{}:{path}", prefix.display()));
        cmd
    }

    /// Pre-seed the standard `ace-rs/school` import cache so the `school
    /// init` PullImports step resolves locally instead of cloning from
    /// GitHub. Adds a `.gitconfig` `insteadOf` so any in-process fetch hits
    /// the process-shared bare origin. The template is built once per test
    /// binary; per-test cost is a small `cp -R`.
    pub fn seed_ace_school_imports(&self) {
        let tpl = ace_school_template();
        let dest = self.path("cache/ace/imports/github.com/ace-rs/school");
        copy_tree(&tpl.root, &dest);
        append_gitconfig_redirect(
            &self.path(".gitconfig"),
            "https://github.com/ace-rs/school.git",
            &tpl.root,
        );
    }

    /// Redirect `https://github.com/<source>.git` to a known-nonexistent
    /// local path so `git clone` fails immediately instead of hitting the
    /// network. Used by tests that assert on clone-failure paths.
    pub fn redirect_to_invalid(&self, source: &str) {
        append_gitconfig_redirect(
            &self.path(".gitconfig"),
            &format!("https://github.com/{source}.git"),
            Path::new("/nonexistent/path"),
        );
    }
}

// -- Shared remote-school template (process-scoped, per specifier) --

struct RemoteSchoolTemplate {
    _tmp: tempfile::TempDir,
    origin: PathBuf,
    cache: PathBuf,
}

fn remote_school_template(specifier: &str) -> std::sync::Arc<RemoteSchoolTemplate> {
    static TEMPLATES: std::sync::OnceLock<
        std::sync::Mutex<HashMap<String, std::sync::Arc<RemoteSchoolTemplate>>>,
    > = std::sync::OnceLock::new();
    let mutex = TEMPLATES.get_or_init(|| std::sync::Mutex::new(HashMap::new()));
    let mut map = mutex.lock().expect("templates lock");
    if let Some(t) = map.get(specifier) {
        return std::sync::Arc::clone(t);
    }
    let t = std::sync::Arc::new(build_remote_school_template(specifier));
    map.insert(specifier.to_string(), std::sync::Arc::clone(&t));
    t
}

fn build_remote_school_template(specifier: &str) -> RemoteSchoolTemplate {
    let tmp = tempfile::TempDir::new().expect("template tempdir");
    let origin = tmp.path().join("origin.git");
    let work = tmp.path().join("work");
    let cache = tmp.path().join("cache");

    std::fs::create_dir_all(&origin).expect("mkdir origin");
    plain_git(
        &origin,
        &["init", "--bare", "--quiet", "--template=", "-b", "main"],
    );

    plain_git(
        tmp.path(),
        &[
            "clone",
            "--quiet",
            origin.to_str().expect("origin utf8"),
            work.to_str().expect("work utf8"),
        ],
    );

    std::fs::write(
        work.join("school.toml"),
        format!("name = \"{specifier}\"\n"),
    )
    .expect("write school.toml");
    std::fs::create_dir_all(work.join("skills/maverick")).expect("mkdir maverick");
    std::fs::write(work.join("skills/maverick/SKILL.md"), "# Maverick\n").expect("write SKILL.md");

    plain_git(&work, &["add", "-A"]);
    plain_git(
        &work,
        &[
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=Test",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    plain_git(&work, &["push", "--quiet"]);

    plain_git(
        tmp.path(),
        &[
            "clone",
            "--quiet",
            origin.to_str().expect("origin utf8"),
            cache.to_str().expect("cache utf8"),
        ],
    );

    RemoteSchoolTemplate {
        _tmp: tmp,
        origin,
        cache,
    }
}

// -- Shared import-cache template (process-scoped) --

/// Process-wide template for the `ace-rs/school` import cache. Built once
/// per test binary via `OnceLock`; each call to `seed_ace_school_imports`
/// copies the template into the per-test XDG_CACHE_HOME so tests stay
/// isolated.
///
/// The template is a single repo that serves as both content and origin:
/// `.git/config` has `remote.origin.url` pointing at the template's own
/// path. Per-test caches inherit that URL on copy — `ace`'s fetch+merge
/// then no-ops against the up-to-date template (no real network).
struct AceSchoolTemplate {
    _tmp: tempfile::TempDir,
    /// Self-referential repo path; also the `insteadOf` target for tests
    /// whose code path falls through to a fresh `git clone`.
    root: PathBuf,
}

fn ace_school_template() -> &'static AceSchoolTemplate {
    static T: std::sync::OnceLock<AceSchoolTemplate> = std::sync::OnceLock::new();
    T.get_or_init(build_ace_school_template)
}

fn build_ace_school_template() -> AceSchoolTemplate {
    let tmp = tempfile::TempDir::new().expect("template tempdir");
    let root = tmp.path().join("school");

    std::fs::create_dir_all(root.join("skills/ace-school")).expect("mkdir ace-school");
    std::fs::write(root.join("school.toml"), "name = \"ace-rs/school\"\n")
        .expect("write school.toml");
    std::fs::write(root.join("skills/ace-school/SKILL.md"), "# ace-school\n")
        .expect("write SKILL.md");

    plain_git(&root, &["init", "--quiet", "--template=", "-b", "main"]);
    plain_git(&root, &["add", "-A"]);
    plain_git(
        &root,
        &[
            "-c",
            "user.email=test@test.com",
            "-c",
            "user.name=Test",
            "commit",
            "--quiet",
            "-m",
            "initial",
        ],
    );
    plain_git(
        &root,
        &[
            "remote",
            "add",
            "origin",
            root.to_str().expect("root path utf8"),
        ],
    );

    AceSchoolTemplate { _tmp: tmp, root }
}

fn plain_git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git")
        .args(args)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(status.success(), "git {args:?} failed in {}", dir.display());
}

fn copy_tree(src: &Path, dst: &Path) {
    if src.is_dir() {
        std::fs::create_dir_all(dst).unwrap_or_else(|e| panic!("mkdir {}: {e}", dst.display()));
        for entry in
            std::fs::read_dir(src).unwrap_or_else(|e| panic!("read_dir {}: {e}", src.display()))
        {
            let entry = entry.expect("dir entry");
            copy_tree(&entry.path(), &dst.join(entry.file_name()));
        }
    } else {
        std::fs::copy(src, dst)
            .unwrap_or_else(|e| panic!("copy {} → {}: {e}", src.display(), dst.display()));
    }
}

fn append_gitconfig_redirect(path: &Path, gh_url: &str, origin: &Path) {
    let block = format!(
        "[url \"file://{}\"]\n\tinsteadOf = {gh_url}\n",
        origin.display(),
    );
    if path.exists() {
        let mut existing = std::fs::read_to_string(path).expect("read gitconfig");
        existing.push_str(&block);
        std::fs::write(path, existing).expect("append gitconfig");
    } else {
        std::fs::write(path, block).expect("write gitconfig");
    }
}
