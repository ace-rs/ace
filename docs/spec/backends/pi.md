# Backend: Pi

Binary: `pi` | Dir: `.pi` | Instructions: `AGENTS.md`

Select with `backend = "pi"`, `ace --pi`, or a named `[backends.<name>]` instance
with `kind = "pi"`. Compatibility baseline: Pi 0.87.1 and pi-mcp-adapter 3.0.0;
see [upstream research](../../vendor/pi-mcp.md).

## Installation prerequisite

Install Pi and register its required adapter using Pi's package manager:

```sh
pi install npm:pi-mcp-adapter@3
```

ACE runs the resolved command's `list --no-approve` operation with the same working
directory and environment as launch, adding `PI_OFFLINE=1` for the probe. The probe
does not approve project resources or load extension factories. A Pi-managed npm
declaration and reported installed directory are required; a version suffix is accepted.
The installed package manifest must identify
`pi-mcp-adapter` at a stable version >=3.0.0 and <4.0.0. Other majors require
compatibility review before ACE can provision their configuration. Missing Pi or adapter
installation exits 2 with recovery guidance. Failed checks and unrecognized output are
separate errors.
Custom command prefixes and the effective Pi agent directory also apply to the recovery
command, including instances that override `HOME`. Command wrappers must transparently
forward Pi arguments; direct Pi command prefixes cannot contain a prompt. Put model and
effort in their backend fields and other launch options in ACE passthrough.
`--mcp-config` may appear in the command prefix; ACE omits it only for package probes.
Use `--mcp-config <path>` or `--mcp-config=<path>` before the `--` separator. ACE rejects
selector tokens used as another option's value or placed after that separator, because
Pi and the adapter interpret those positions differently. A literal `--` used as an
option value is rejected for the same reason.
Project-only, Git, local, and autodiscovered extension installations are not certified
by this user-scope npm installation check.

This proves package installation, not extension loading, project trust, authentication,
or MCP health. Explicit extension-disabling flags are rejected; runtime filtering and
load failures remain Pi diagnostics. Configure resource selection through `pi config`.
ACE never installs packages, grants trust, or completes OAuth during this check.

Setup checks the prospective merged configuration before publishing `ace.toml`.
Preparation acquires the school, refreshes bindings, and checks the final invocation
before linking skills or writing configuration. Explicit MCP registration checks before
clearing an exclusion. Pure ACE configuration inspection needs no Pi installation.

## Launch and skills

ACE maps `model` to `--model`, `effort` to `--thinking`, and latest-session resume to
`--continue`. A fresh session supplies its instructions with `--append-system-prompt`
using a temporary file, avoiding Pi's ambiguity between literal text and a file path.
One-shot requests use `--print`; literal prompts travel through stdin so leading `@`
or `-` cannot become attachments or options. Passthrough arguments follow ACE arguments.

Sessions use ACE's shared process supervision and preserve child exit status. Pi's
resource approval is distinct from tool permission modes: ACE warns for `auto` or
`yolo`, and does not translate them into `--approve`.

Pi consumes nested `.pi/skills` and `AGENTS.md`. Other canonical school folders retain
ACE's compatibility linking behavior with unsupported-folder warnings. Existing
`.agents/skills` content remains subject to normal reconciliation rules.

## MCP configuration

ACE writes the adapter's user-scope `mcp-adapter.json`, under `PI_CODING_AGENT_DIR` or
`~/.pi/agent`; an explicit `--mcp-config` selects that layer's alternative path.
It preserves unrelated JSON fields and existing server definitions. OAuth, credential
storage, project trust, and server approval remain adapter-owned. Use `/mcp-adapter`
inside Pi for approval and `/mcp-auth <name>` for OAuth; OAuth is manual by default.

Inventory is fallible and retains the effective source of each name. Earlier shared
user layers and later project layers participate in precedence; an existing name is
never overwritten by registration. Removing a user definition reports any remaining
definition and its source. Reset applies the same rule to school-defined names.

Ancestor discovery is disabled by default. A user-global configuration or the explicitly
selected config can enable `settings.ancestorConfigRoots` with absolute or `~/...`
directory paths under the user's home. ACE selects the nearest root containing the
canonical project directory and reads ancestor `.mcp.json` and `.pi/mcp-adapter.json`
files from that root toward the project; nearer layers override farther ones, and the
project's own files win. Project configuration cannot enable or extend this boundary.

Malformed configuration and active sources ACE cannot safely inventory cause explicit
errors. ACE does not silently treat them as an empty configuration. JSON with comments
must be converted to plain JSON before ACE can edit it; no comments are discarded.
Header values are literal credentials: adapter shell-command syntax is escaped, and
unsupported interpolation is rejected without echoing the credential.

Nonempty `imports`, `claudePlugins`, `agentPluginPaths`, enabled host discovery,
package-declared `pi.mcp` servers, and project package declarations cannot currently be
inventoried by ACE. It refuses these configurations with a diagnostic rather than
overwriting names supplied by an unaccounted source. Rebranded Pi (`PI_PACKAGE_DIR`)
and undocumented `PI_MCP_CONFIG_MODE` overrides are also unsupported.

## Health checks

Only an explicit `ace mcp check` starts a health-check model session. It uses the selected
Pi command, environment, model, and effort, with live output, and asks the adapter to
connect and discover the named servers without invoking arbitrary discovered tools.
The adapter retains its configured authentication policy; `autoAuth` may initiate
authentication despite the diagnostic prompt asking not to. Configuration presence
alone is never success. Missing or malformed results fail the check; these are
model-mediated diagnostics, not a deterministic standalone adapter probe. Startup never
performs a paid health check.

Health checks require Pi's text output mode; an explicit JSON or RPC mode is rejected
rather than overridden. The model must report every requested server exactly once, and
an unhealthy result makes the command fail.
