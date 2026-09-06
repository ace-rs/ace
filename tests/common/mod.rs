#![allow(dead_code)]

mod fixtures;
pub use fixtures::*;

use assert_cmd::Command;
use std::path::Path;

impl TestEnv {
    /// Create an embedded school and run `ace setup .` — the most common test fixture.
    pub fn setup_embedded(&self, name: &str) {
        self.git_init();
        self.setup_embedded_school(name);
        self.ace().args(["setup", "."]).assert().success();
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
}
