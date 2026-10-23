use std::collections::HashMap;

use crate::ace::Ace;
use crate::templates::{Template, builtins};

/// `ace llm-help` — the capability catalog for agents. Prose sections are compiled into
/// the binary; the command and flag lists are rendered from the clap definitions, so the
/// catalog always matches the running version (docs/spec/ux.md § 2, `ace llm-help`).
pub fn run(ace: &mut Ace, cli: &clap::Command) {
    ace.data(&render(cli));
}

fn render(cli: &clap::Command) -> String {
    let vals = HashMap::from([
        ("commands".to_string(), render_commands(cli)),
        ("flags".to_string(), render_flags(cli)),
    ]);
    Template::parse(builtins::LLM_HELP)
        .substitute(&vals)
        .trim_end()
        .to_string()
}

fn render_commands(cli: &clap::Command) -> String {
    let lines: Vec<String> = visible_subcommands(cli)
        .flat_map(|cmd| {
            let top = command_line("", cmd);
            let nested = visible_subcommands(cmd).map(|sub| command_line(cmd.get_name(), sub));
            std::iter::once(top).chain(nested.map(|line| format!("  {line}")))
        })
        .collect();
    lines.join("\n")
}

fn render_flags(cli: &clap::Command) -> String {
    let lines: Vec<String> = cli
        .get_arguments()
        .filter(|arg| !arg.is_hide_set())
        .filter_map(|arg| {
            let long = arg.get_long()?;
            let about = arg.get_help().map(|h| h.to_string()).unwrap_or_default();
            Some(format!("- `--{long}` — {}", first_line(&about)))
        })
        .collect();
    lines.join("\n")
}

fn visible_subcommands(cmd: &clap::Command) -> impl Iterator<Item = &clap::Command> {
    cmd.get_subcommands().filter(|sub| !sub.is_hide_set())
}

fn command_line(parent: &str, cmd: &clap::Command) -> String {
    let path = match parent {
        "" => format!("ace {}", cmd.get_name()),
        parent => format!("ace {parent} {}", cmd.get_name()),
    };
    let about = cmd.get_about().map(|a| a.to_string()).unwrap_or_default();
    let aliases: Vec<&str> = cmd.get_visible_aliases().collect();

    match aliases.as_slice() {
        [] => format!("- `{path}` — {}", first_line(&about)),
        aliases => format!(
            "- `{path}` (alias: {}) — {}",
            aliases.join(", "),
            first_line(&about)
        ),
    }
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default().trim()
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;
    use crate::cmd::Cli;

    #[test]
    fn catalog_lists_every_visible_command() {
        let cli = Cli::command();
        let out = render(&cli);

        for cmd in visible_subcommands(&cli) {
            let path = format!("`ace {}`", cmd.get_name());
            assert!(out.contains(&path), "catalog misses {path}");
        }
        assert!(
            out.contains("`ace school init`"),
            "nested commands are listed"
        );
        assert!(!out.contains("{{ commands }}") && !out.contains("{{ flags }}"));
    }
}
