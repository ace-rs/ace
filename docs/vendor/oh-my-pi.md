<!-- derived from: https://github.com/can1357/oh-my-pi @ v18.6.0; read 2026-10-04 -->

# Oh My Pi: comparison with Pi for ACE maintainers

[Oh My Pi][project] is the `can1357/oh-my-pi` fork of Mario Zechner's Pi,
distributed as `omp`; it is a separate application, not an extension bundle installed
into regular Pi. [Pi][pi] emphasizes a minimal extensible harness, while OMP includes
LSP, debugger integration, subagents, built-in MCP, and additional workflow tools.
These are upstream feature descriptions, not independently measured quality claims.

The latest stable release observed was [v18.6.0][release], published on
2026-10-03 at 22:06 UTC. Its [package manifest][package] declares
`@oh-my-pi/pi-coding-agent` and the `omp` executable. Upstream documents Homebrew,
prebuilt binaries, Bun, and Nix distribution; this investigation installed nothing.

## Compatibility with ACE's implemented Pi backend

The ACE column describes [ACE's Pi contract][ace-pi], not a promise about every
version of regular Pi. An approved change in progress removes the adapter requirement
from ordinary launch and verifies installation at the MCP inventory boundary; the
table reflects that intended boundary without claiming the change has landed.
OMP observations use v18.6.0 unless the linked source explicitly says otherwise.

| Surface          | ACE Pi behavior                                               | OMP behavior                                                    | Compatibility consequence                                        |
| ---------------- | ------------------------------------------------------------- | --------------------------------------------------------------- | ---------------------------------------------------------------- |
| Executable       | `pi`; named instances may replace the command                 | `omp`                                                           | Command selection alone is insufficient                          |
| Launch           | Model, effort, resume, appended prompt file, print mode/stdin | Same basic launch flags and stdin prompt surface                | Reuse argument/process machinery after semantic review           |
| MCP prerequisite | MCP-only `list --no-approve`; verifies pi-mcp-adapter 3.x     | `omp list` is rejected; management uses `omp plugin list`       | MCP probe incompatible; native MCP needs no Pi adapter           |
| Instructions     | Root `AGENTS.md`                                              | Root `AGENTS.md` supported, with competing source precedence    | Reuse emission; verify which context file actually wins          |
| Skills           | Nested `.pi/skills/<identity>/SKILL.md`                       | Flat `.omp/skills/<name>/SKILL.md`; no recursive scan           | Different directory and capability; reuse ACE's flattening       |
| MCP persistence  | Adapter user `mcp-adapter.json`                               | Native project/user `mcp.json`, with named-profile relocation   | New native persistence and effective-path handling required      |
| MCP inventory    | Adapter layers, ancestor boundary, unsupported-source errors  | Multiple providers, profile scopes, source and endpoint dedup   | Pi's effective inventory cannot be reused unchanged              |
| Authentication   | Pi provider auth; adapter-owned MCP OAuth                     | Native provider/MCP auth store; `/login`, `/mcp reauth`         | Leave credentials to OMP; use its recovery commands              |
| MCP health       | Model calls adapter proxy `mcp` with `connect`                | Native connected tools and `/mcp test`; no adapter proxy        | Existing prompt is invalid; need a supported live status surface |
| Tool trust       | Pi supports only ACE's default trust mapping                  | `--approval-mode` with always-ask, write, or yolo; default yolo | Explicit mapping review needed before advertising ACE trust      |

Launch evidence: [CLI reference][cli] documents `--model`, `--thinking`,
`--continue`, `--append-system-prompt`, `--print`, and literal stdin prompts.
The [command table][commands] explicitly rejects top-level `list` and directs
users to `omp plugin list`; Pi's inventory text and offline probe are not contracts
OMP promises. [Approval documentation][approval] describes the tool modes.

Discovery evidence: [configuration reference][config] excludes `.pi` from generic
discovery and explains `PI_CONFIG_DIR`, `PI_CODING_AGENT_DIR`, named profiles,
and XDG relocation. [Skill documentation][skills] specifies immediate children
only; [the scanner][scanner] confirms that it checks each child's `SKILL.md`
without recursive enumeration. [Context documentation][context] explains how a
native or foreign higher-priority file can shadow standalone `AGENTS.md`.

MCP evidence: [configuration guide][mcp] specifies `.omp/mcp.json` and the active
profile's user `mcp.json`, including project-over-user and cross-provider precedence.
User enable/disable overrides and equivalent-connection deduplication also affect
which servers are effective. Literal credential handling needs separate review:
native definitions can expand environment placeholders, and header/env strings may
invoke `!command` or resolve as environment-variable names.

[Provider documentation][providers] places local credentials in active-profile
`agent.db`, or a configured broker, and documents terminal `omp login`.
[MCP authoring documentation][mcp-authoring] describes native OAuth and `/mcp test`;
[runtime documentation][runtime] distinguishes live handshakes from cached/deferred
tools. Print mode has a readiness barrier and `OMP_MCP_REQUIRE_READY=1`, but that
checks all configured servers before a model turn, not a requested-name health API.

## Assessment and unresolved boundaries

**Assessment:** full ACE support warrants a distinct backend kind, because directory,
skill capability, prerequisites, persistence, and health contracts differ. A named
instance with `kind = "pi"` and `cmd = "omp"` cannot provide correct skill discovery
and MCP support, despite the substantial overlap in launch flags and removal of
the ordinary-launch adapter requirement. This is an engineering inference from the
observed contracts, not an approved implementation decision.

Existing ACE capabilities worth reusing are named-instance resolution, process
supervision, temporary instruction/prompt files, and capability-driven skill
flattening. Pi's package verifier and adapter configuration policy should remain
Pi-specific; similarity of JSON fields does not establish identical behavior.

No deterministic standalone OMP CLI was established for both effective server
inventory and live health by requested name. `omp read mcp://...` resolves server
resources, so it is not a general substitute for health checks. Upstream SDK and
interactive MCP surfaces exist, but ACE should not reach into private internals or
introduce a shim without a specific sanctioned interface and an explicit ruling.

OMP retains some legacy Pi extension loading compatibility, but [its porting guide][port]
documents intentional API and credential-store divergence. Installing Pi's adapter
into OMP is therefore neither necessary for native MCP nor verified by this research.
No binary was launched, model invoked, OAuth completed, or live configuration tested;
runtime behavior, older OMP releases, and exact profile/XDG edge cases remain unverified.

[project]: https://github.com/can1357/oh-my-pi
[pi]: https://github.com/earendil-works/pi/tree/main/packages/coding-agent
[release]: https://github.com/can1357/oh-my-pi/releases/tag/v18.6.0
[package]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/packages/coding-agent/package.json
[ace-pi]: ../spec/backends/pi.md
[cli]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/cli-reference.md
[commands]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/packages/coding-agent/src/cli-commands.ts
[approval]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/approval-mode.md
[config]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/config-usage.md
[skills]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/skills.md
[scanner]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/packages/coding-agent/src/discovery/helpers.ts
[context]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/context-files.md
[mcp]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/mcp-config.md
[providers]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/providers.md
[mcp-authoring]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/mcp-server-tool-authoring.md
[runtime]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/mcp-runtime-lifecycle.md
[port]: https://github.com/can1357/oh-my-pi/blob/v18.6.0/docs/porting-from-pi-mono.md
