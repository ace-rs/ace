<!-- derived from: this machine's installs, read 2026-08-01 -->

# Backend install and upgrade

How each backend ACE dispatches to gets onto a machine, and how it moves versions.
ACE never installs a backend; this is here so a version question has an answer without
re-deriving it from `which` output.

Each backend owns its own update story, and they disagree — two self-update in place,
two are managed by a package manager. That mismatch matters whenever a backend's
behavior changes under you mid-session.

| Backend    | Install                                 | Upgrade                     | Lands at                             |
| ---------- | --------------------------------------- | --------------------------- | ------------------------------------ |
| `claude`   | native installer                        | `claude update` (self)      | `~/.local/share/claude/versions/<v>` |
| `codex`    | Homebrew **cask** `codex`               | `brew upgrade --cask codex` | `/opt/homebrew/Caskroom/codex/<v>/`  |
| `opencode` | Homebrew **formula** `opencode`         | `brew upgrade opencode`     | `/opt/homebrew/Cellar/opencode/<v>/` |
| `hermes`   | `uv tool install --editable <checkout>` | `hermes update` (git-pull)  | the checkout, via a uv-owned shim    |

Shims land in `~/.local/bin`: `claude` for the native installer, and
`{hermes,hermes-acp,hermes-agent}` for the uv tool.

Versions observed 2026-08-01: `claude` 2.1.220 · `codex-cli` 0.145.0 · `opencode` 1.18.5
· `hermes-agent` 0.19.1 (upstream `e444d165`).

## Hermes runs from a checkout through a uv shim

Hermes is a Python app with no compiled artifact — the thing on `$PATH` is a console
script whose shebang points into a virtualenv, so there is nothing to copy to `~/bin`.

On this machine the two halves come from different places. `uv tool` owns the venv and the
`~/.local/bin` shims, but it was installed **editable**: site-packages holds only an
`__editable__.hermes_agent-*.pth` finder, and every import resolves into
`~/Documents/chakrit/hermes-agent`. So `uv tool list` reports the version recorded at
install time while `hermes --version` reports the checkout's current commit, and the two
drift apart on every `git pull`.

That also settles which updater applies. `hermes update` (`hermes_cli/main.py`,
`cmd_update`) git-pulls the install tree when the install is a checkout; install method
comes from a `.install_method` stamp next to the running code, falling back to `.git`
detection (`hermes_cli/config.py`, `detect_install_method`). There is no stamp here, the
running code sits in a clone, so detection reports `git` and `hermes update` pulls that
clone. `uv tool upgrade hermes-agent` is the wrong lever — it would rebuild the shim
against the same checkout.

Consequence for ACE: a hermes version can change under a running session with no package
manager involved, and `hermes --version` describes whichever copy was invoked.

## Building from source instead

`scripts/harnesses.sh` builds a backend from upstream source into a disposable
repo-local pen when you need a pinned copy rather than whatever your machine has —
see `../guides/harnesses.md`.
