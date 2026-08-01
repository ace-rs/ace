<!-- derived from: this machine's installs, read 2026-08-01 -->

# Backend install and upgrade

How each backend ACE dispatches to gets onto a machine, and how it moves versions.
ACE never installs a backend; this is here so a version question has an answer without
re-deriving it from `which` output.

Each backend owns its own update story, and they disagree — two self-update in place,
two are managed by a package manager. That mismatch matters whenever a backend's
behavior changes under you mid-session.

| Backend    | Install                              | Upgrade                      | Lands at                                  |
| ---------- | ------------------------------------ | ---------------------------- | ----------------------------------------- |
| `claude`   | native installer                     | `claude update` (self)       | `~/.local/share/claude/versions/<v>`, shim `~/.local/bin/claude` |
| `codex`    | Homebrew **cask** `codex`            | `brew upgrade --cask codex`  | `/opt/homebrew/Caskroom/codex/<v>/`       |
| `opencode` | Homebrew **formula** `opencode`      | `brew upgrade opencode`      | `/opt/homebrew/Cellar/opencode/<v>/`      |
| `hermes`   | `uv tool install` (`hermes-agent`)   | `uv tool upgrade hermes-agent` | `~/.local/bin/{hermes,hermes-acp,hermes-agent}` |

Versions observed 2026-08-01: `claude` 2.1.220 · `codex-cli` 0.145.0 · `opencode` 1.18.5
· `hermes-agent` 0.19.0.

## Hermes has two update paths, and they fight

Hermes is a Python app with no compiled artifact — the thing on `$PATH` is a console
script whose shebang points into a virtualenv, so there is nothing to copy to `~/bin`.
`uv tool install` gives it a managed venv of its own and shims in `~/.local/bin`.

It also ships its own updater: a top-level `hermes update` (`hermes_cli/main.py`,
`cmd_update`) which git-pulls the install tree when the install is a checkout. Install
method is read from a `.install_method` stamp in that tree, with fallback detection
(`hermes_cli/config.py`, `detect_install_method`) — which is why `hermes --version` run
against a clone reports `Install method: git`.

A `uv tool` install has no `.git` in its install tree, so detection cannot report `git`
there. **Which branch `hermes update` then takes is untraced** — do not assume it is a
no-op. Upgrade through `uv tool upgrade hermes-agent`, and treat a version from
`hermes --version` as describing whichever copy you invoked.

The same split shows up if a source checkout also exists: `uv tool list` and a checkout's
`.venv/bin/hermes` are independent installs that drift.

## Building from source instead

`scripts/harnesses.sh` builds a backend from upstream source into a disposable
repo-local pen when you need a pinned copy rather than whatever your machine has —
see `../guides/harnesses.md`.
