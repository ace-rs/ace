```
░█▀█░█▀▀░█▀▀
░█▀█░█░░░█▀▀
░▀░▀░▀▀▀░▀▀▀
```

**ACE** (Accelerated Coding Environment) — automation tooling for setting up and keeping AI coding
environments up-to-date. Acts as an entrypoint to supported AI coding backends such as
[Claude Code](https://docs.anthropic.com/en/docs/claude-code) and Codex.

## Install

**curl installer** (recommended):

```sh
curl -fsSL https://ace-rs.dev/install.sh | bash
```

**GitHub release** (manual):

Download the binary for your platform from the
[latest release](https://github.com/ace-rs/ace/releases/latest), `chmod +x`, and move to
somewhere on your `$PATH`.

**Source** (development):

```sh
cargo install --path .
```

## Usage

```sh
ace setup ace-rs/school                       # clone a school, register MCP, write config
ace                                          # launch the configured backend
ace --codex                                  # temporarily use Codex for this invocation
ace -- --continue                            # pass flags through to the backend
ace mcp                                      # register/check school MCP servers
ace pull                                     # fetch latest school changes and relink
ace import anthropics/skills --skill commit  # import a skill from an external repo
ace school update                            # re-fetch all imported skills
```

## Commands

| Command | Description |
|---------|-------------|
| `ace setup [specifier]` | Clone a school, register MCP servers, write config |
| `ace pull` | Fetch latest school changes and relink project folders |
| `ace config` | Print effective configuration |
| `ace paths [key]` | Print resolved filesystem paths (e.g. `ace paths school`) |
| `ace mcp` | Add missing MCP servers, health-check, and help re-register broken ones |
| `ace mcp check` | Health-check registered MCP servers without mutating state |
| `ace mcp reset [name]` | Remove registered MCP servers so they can be re-added cleanly |
| `ace import <source> [--skill <name>] [--all]` | Import a skill from an external repository (`--skill` accepts globs like `frontend-*`; `--all` imports every curated skill) |
| `ace school init` | Initialize a new school repository |
| `ace school update` | Re-fetch all imported skills from their sources |
| `ace school skills` | List skills in the current school |
| `ace diff` | Show uncommitted changes in the school cache |
| `ace auto` | Persist auto trust mode in `ace.local.toml` |
| `ace yolo` | Persist yolo trust mode in `ace.local.toml` |

## How it works

ACE manages **schools** — shared repositories of skills, conventions, and configuration for AI
coding tools. When you run `ace`, it:

1. Resolves which school to use (from `ace.toml`)
2. Fetches/updates the school repository
3. Symlinks skills into your project
4. Launches the configured backend with the school's session prompt

Backend selection can also be overridden per invocation with `-b`, `--backend`,
`--claude`, `--codex`, or `--flaude`.

## School workflow

Schools contain shared folders (`skills/`, `rules/`, `commands/`, `agents/`). When you run
`ace`, each folder present in the school is symlinked into your project — everyone on the same
school works against the same files.

**First-time setup with existing folders:** If your project already has a real `skills/` (or
any of the four folders), ACE moves it to `previous-skills/` on first run. The LLM will then
help you merge the contents into the school.

**Changing school files:** Edit through symlinks (edits go to the school cache directly). The
AI backend handles proposing changes back — branch, commit, push, create PR via GitHub MCP.

**Parent school pattern:** Inherit every skill from another repository with `ace import <source> --all`.
This records `skill = "*"` in `[[imports]]`; subsequent `ace school update` runs re-fetch and pick up
new skills automatically. The source doesn't need to be an ACE school — any repo with a `skills/`
folder works, including `anthropics/skills` and community [skills.sh](https://skills.sh)-style
collections.

```sh
ace import company/school --all       # inherit a company-wide school
ace import anthropics/skills --all    # or pull from a plain skills repo
ace school update                     # refresh everything
```

## Configuration

- `ace.toml` — project-level config (school specifier, backend, env)
- `ace.local.toml` — local overrides (gitignored)
- `~/.config/ace/config.toml` — user-level config (credentials)
- `school.toml` — school metadata (name, MCP servers, projects)

## Development

```sh
cargo test              # unit tests + integration tests (no network required)
cargo test --test setup_test  # run a single test file
```

Integration tests live in `tests/` and use `TestEnv` (tempdir sandbox + `assert_cmd`). Each
test file covers one CLI command. Tests that require network (clone) are not yet supported —
see ROADMAP.

## Releases & cross-build

See [RELEASE.md](RELEASE.md) — the canonical runbook for cutting releases,
cross-building binaries, prereqs, Homebrew, the `latest` marker, and the
website notification step.

## License

MIT
