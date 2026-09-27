<!-- derived from: upstream sources linked below @ 2026-09-27 -->

# Pi MCP integration comparison

For ACE maintainers assessing Pi support. This is a documentation and source review,
not a runtime compatibility certification or an approved implementation design.

## Findings

Both extension-based MCP and MCPorter CLI access are viable integration approaches.
They share a server-definition concept, but not one interchangeable configuration,
authentication store, or activation mechanism.

The previously suggested `pi-mcporter` extension is now explicitly unmaintained.
Its author recommends `pi-mcp-client`; MCPorter itself remains a separate CLI/runtime.
The original Pi integration draft's blanket exclusion of MCP is therefore too narrow,
but adopting the discontinued bridge as a new dependency is also a poor default.

| Surface          | pi-mcp-adapter               | MCPorter + pi-mcporter             | pi-mcp-client                    |
| ---------------- | ---------------------------- | ---------------------------------- | -------------------------------- |
| Status checked   | Adapter v3.0.0 tag           | CLI current; bridge unmaintained   | Bridge author's successor        |
| Remote support   | Streamable HTTP and SSE      | Streamable HTTP and SSE            | Streamable HTTP; rejects SSE     |
| Tool interface   | Proxy or direct tools        | CLI; bridge proxy/native tools     | Discovery and native activation  |
| Management       | Documented JSON, Pi UI       | CLI config commands                | Documented JSON, Pi commands     |
| OAuth owner      | Adapter credential store     | MCPorter vault                     | OS keyring through extension     |

## Pi MCP Adapter

[v3.0.0 README][adapter] and [OAuth guide][adapter-auth] document remote URLs, headers,
environment interpolation, authentication, and proxy/direct exposure.
User-wide adapter configuration is `<Pi agent dir>/mcp-adapter.json`; shared global
files and project `.mcp.json` also participate, with `.pi/mcp-adapter.json` overriding
project settings. The Pi directory honors `PI_CODING_AGENT_DIR`.

The v3 [changelog][adapter-changes] reserves `mcp.json` for *upcoming* Pi MCP support,
renames the UI command to `/mcp-adapter`, and stops reading the former adapter paths.
This is not evidence that the checked Pi v0.87.1 core already provides native MCP.

The public `--mcp-config` flag replaces the global adapter layer; project configuration
can still override it. Do not treat it as complete isolation. An exclusive-mode
environment switch found in implementation lacks a documented public contract and
should not be an ACE dependency.

Project servers require trust and server approval; unattended sessions skip unapproved
servers unless the documented user-global policy permits them. The helper CLI offers
initialization and credential operations, not a complete server-management API.
ACE would need loss-averse edits through the documented JSON surface.

## MCPorter and the discontinued Pi bridge

[MCPorter configuration][porter-config] supports home/XDG and project JSONC, with
project entries overriding home entries. `--config` or `MCPORTER_CONFIG` selects an
explicit file; imports remain a separate input and need explicit disabling for isolation.
The CLI provides `config list/get/add/remove/doctor`, user-scope additions, and OAuth
login. Imported definitions are read-only; removing a local definition can expose an
imported definition again. `config add` replaces an existing same-named entry, so ACE's
existing-name check must precede it.

The [CLI/runtime][porter] works independently of a Pi extension through ordinary shell
tools. This is portable CLI access, not automatically native Pi tool registration.
OAuth remains in MCPorter's vault. A live `list` operation probes servers; it is not
the cheap configuration inventory required by bare `ace mcp`.

The [discontinued bridge][bridge] shares MCPorter configuration and credentials but
loads its own library version, which can differ from the installed CLI. It has a Pi
exposure policy separate from server configuration. Documentation establishes project
trust for exposure settings, but this review did not establish that Pi trust gates all
MCPorter project-server imports. Do not assume that boundary.

## Pi MCP Client successor

The [successor README][client] requires Pi >=0.85.1 and Node >=22.
[Configuration][client-config] uses `<Pi agent dir>/mcp.json` and trusted project
`.mcp.json`, with full-entry project replacement. It accepts URL/header definitions
and environment references, but rejects legacy SSE and unsupported fields.

[Authentication][client-auth] is explicit through `/mcp login`; the model cannot start
a browser login. It supports public OAuth clients and OS-keyring storage, not
confidential clients requiring a secret. Remote manual completion is supported, but
print/JSON mode cannot perform interactive login. Existing credentials or bearer
headers are distinct from first-time authorization.

[Behavior][client-behavior] documents lazy discovery, native activation, and explicit
project trust. No standalone management CLI or arbitrary configuration-path override
was established in this review. Documented JSON is the provisioning boundary.

## Implications for ACE — recommendations, not settled design

- Keep school server declarations backend-neutral. Bind MCP operations to the selected
  backend instance's MCP provider, preserving its command/environment and config context.
  Existing `Backend` and `McpDecl` provide the reusable boundaries; MCP dispatch solely
  through `Kind` cannot express two Pi instances selecting different providers.
- Use explicit provider selection, not “whichever package happens to be installed.”
  Support for multiple providers should not register every server into every provider.
  Treat extension loading and provisioning as separate concerns: an ACE selector alone
  cannot prevent another installed extension from discovering the same shared file.
- Preserve ACE's user-scope registration contract and existing entries. Write only to
  the selected provider's documented configuration; leave OAuth to that provider.
  Report imports, overrides, disabled entries, unsupported capabilities, and unavailable
  integrations rather than flattening all of them into an empty successful inventory.
- Keep configuration inspection separate from runtime usability. A successful MCPorter
  probe does not establish that Pi loaded an extension or can use its tools; health
  evidence must exercise the selected Pi path with visible diagnostics.

Recommended direction: support the adapter and standalone MCPorter as selectable
approaches if both native tools and CLI access are wanted. If the requirement is two
maintained native Pi integrations, evaluate `pi-mcp-client` as the second provider
instead of the discontinued `pi-mcporter` bridge. Its SSE limitation is material to
ACE's existing remote-server contract, so it cannot silently substitute for MCPorter.

Before promising compatibility, use isolated fixtures to verify effective config
precedence, preservation on add/remove, missing-provider behavior, duplicate loading,
and Pi-side tool discovery. Test OAuth separately with an agreed account and endpoint.
No packages were installed, credentials accessed, model calls made, application code
changed, or external state updated for this assessment.

## Provenance

Adapter v3.0.0 resolves to `3a13257ca62db6995b5cc78bba9ba1a872d72607`.
Other inspected main revisions and manifest versions are snapshots, not verified npm
release claims:

- MCPorter: `5b3bd5c774361dc2565a066fb1e62d9fa8d3e8b7`, manifest 0.14.1.
- Pi bridge: `337abe0e94730554a33f6496e7ee5bad48f2875c`, manifest 1.0.2.
- Pi MCP Client: `1460408f40952fef8b8c99466ebc95c3766d2e53`, manifest 0.10.0.

[adapter]: https://github.com/nicobailon/pi-mcp-adapter/blob/v3.0.0/README.md
[adapter-auth]: https://github.com/nicobailon/pi-mcp-adapter/blob/v3.0.0/OAUTH.md
[adapter-changes]: https://github.com/nicobailon/pi-mcp-adapter/blob/v3.0.0/CHANGELOG.md
[porter-config]: https://github.com/openclaw/mcporter/blob/5b3bd5c774361dc2565a066fb1e62d9fa8d3e8b7/docs/config.md
[porter]: https://github.com/openclaw/mcporter/blob/5b3bd5c774361dc2565a066fb1e62d9fa8d3e8b7/README.md
[bridge]: https://github.com/mavam/pi-mcporter/blob/337abe0e94730554a33f6496e7ee5bad48f2875c/README.md
[client]: https://github.com/mavam/pi-mcp-client/blob/1460408f40952fef8b8c99466ebc95c3766d2e53/README.md
[client-config]: https://github.com/mavam/pi-mcp-client/blob/1460408f40952fef8b8c99466ebc95c3766d2e53/docs/configuration.md
[client-auth]: https://github.com/mavam/pi-mcp-client/blob/1460408f40952fef8b8c99466ebc95c3766d2e53/docs/authentication.md
[client-behavior]: https://github.com/mavam/pi-mcp-client/blob/1460408f40952fef8b8c99466ebc95c3766d2e53/docs/behavior.md

## Installation probe evidence

Pi 0.87.1 documents `pi list --no-approve` as an inventory that ignores project-local
files in its [package command help][pi-package-cli]. Its implementation bypasses
project-trust extension loading when this explicit override is supplied, then prints
user package declarations and an indented path for each existing installation.
The [package manager][pi-package-manager] establishes that a printed path means the
installation path exists; it does not establish successful loading or authentication.
There is no documented JSON output contract, so an unrecognized format must fail
verification rather than imply the adapter is absent.

An ordinary `pi list` can load global extension factories while resolving project
trust. That resource resolution can install missing packages, and extension factories
can start connections. ACE therefore uses `list --no-approve` with `PI_OFFLINE=1` and
closed stdin for this check: the explicit trust override avoids factory loading,
while Pi's offline setting also blocks automatic package installation.
This intentionally verifies a user-scope npm adapter installation; a project-only,
Git, local-path, or autodiscovered extension is not established by that check.

`pi --help` is unsuitable as a stronger readiness check: it prints registered extension
flags but exits successfully before handling extension-load diagnostics in
[Pi's entry point][pi-entry]. Loading an adapter flag therefore cannot certify runtime
readiness, server authentication, or connectivity.

ACE also verifies the listed adapter's manifest name and stable major version 3,
because adapter 2 uses different configuration filenames. Missing installation files
require reinstalling `npm:pi-mcp-adapter@3`; malformed manifests fail verification.
This does not certify that an extension factory will load successfully.

Configured command wrappers must transparently forward ACE's arguments to Pi without
inserting prompts, launch flags, or subcommands. ACE rejects positional arguments for
a direct `pi` command and strips the documented adapter `--mcp-config` selector only
from the inventory probe; it cannot inspect the behavior of an arbitrary wrapper.
Unexpected project-package output fails verification instead of extending the supported
user-scope installation contract.

[pi-package-cli]: https://github.com/earendil-works/pi/blob/v0.87.1/packages/coding-agent/src/package-manager-cli.ts
[pi-package-manager]: https://github.com/earendil-works/pi/blob/v0.87.1/packages/coding-agent/src/core/package-manager.ts
[pi-entry]: https://github.com/earendil-works/pi/blob/v0.87.1/packages/coding-agent/src/main.ts
