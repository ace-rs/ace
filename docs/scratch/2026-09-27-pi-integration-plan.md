<!-- not spec/decision because: execution checklist; product contract is docs/spec/backends/pi.md -->

# Pi integration plan

Scope: native Pi backend with required `pi-mcp-adapter`, selected by Chakrit.
Owner: [Outline A — Backends](https://outline.prodigy9.co/doc/a-backends-bgUEggvFml),
`pi-harness-support`. [Research and compatibility limits](../vendor/pi-mcp.md).
Documentation baselines: Pi 0.87.1 and adapter 3.0.0; runtime verification remains pending.

## User behavior

Support `backend = "pi"`, `ace --pi`, named instances, nested `.pi/skills`, and `AGENTS.md`.
Reuse the registry, skill reconciliation, and `SessionProcess`; add Pi argument translation:
model → `--model`, effort → `--thinking`, latest → `--continue`, session instructions →
`--append-system-prompt`, one-shot → `--print`. Preserve fresh sessions, cwd, command
prefixes, environment, passthrough precedence, and child exit status.

Require the adapter; leave installation, project trust, and OAuth to the user/Pi.
Report unsupported ACE trust modes without translating them into Pi's `--approve`.
Managed sessions, additional MCP providers, and dependency changes are outside this slice.

## Installation check and errors

Run the resolved Pi command's `list --no-approve` operation in the launch environment
with `PI_OFFLINE=1` to avoid trust prompts and implicit package installation; require the
Pi-managed `npm:pi-mcp-adapter` declaration, including versioned forms, and its reported
installed path. Validate this parser against the baseline CLI output before implementation;
`pi list` has no documented machine-readable inventory contract.

Missing declaration/files: exit 2 with the applicable `pi install npm:pi-mcp-adapter@3`
command and retry hint. Missing Pi: identify the executable and installation guidance.
Check failure or unrecognized output: retain diagnostics and say verification failed,
not “not installed.” Respect custom commands and `PI_CODING_AGENT_DIR` in recovery hints.

This verifies Pi-managed installation, not successful extension loading. Reject explicit
extension-disabling launch flags; keep Pi's runtime load errors visible and direct users
to `pi config` for filtering issues. Git/local/autodiscovered installs need separate
recognition before support is claimed; an unrecognized source must not be called absent.
Do not use `--help` as a probe: extension factories can start connections and its success
status does not establish clean loading. No model calls, installation, or OAuth in preflight.

## Shared invocation context and mutation order

Make `Backend` the single owner of effective invocation context: configured executable
and prefix, project directory, inherited environment overlaid by resolved ACE environment
then backend environment, model/effort, and applicable launch arguments. Pass that context
through preflight, MCP inventory/add/remove/check, and launch; replace MCP dispatch that
currently drops instance fields. Resolve adapter paths from that same environment.

Acquire/validate the school first, then invalidate stale bindings and resolve the final
school-derived backend. During setup, use the pending school assignment in memory through
the existing config merge/binding pipeline, without publishing `ace.toml` to resolve it.
Run preflight before setup publishes `ace.toml`, preparation links files or writes
configuration, or `ace mcp register` clears an exclusion. School cache acquisition may
precede the check; project configuration must remain unchanged on failure.
Recheck if the effective invocation changes; keep pure config inspection independent.

## MCP lifecycle

Use adapter v3's documented user-scope `mcp-adapter.json`; preserve existing content.
Inventory must be fallible and retain source/layer provenance, honoring adapter precedence
and explicit config selection. Malformed files are errors, not empty inventories.
Skip registration when the name exists in any effective layer. Remove only the selected
user-scope definition; if another layer supplies it, report that source and the remaining
registration rather than claiming removal. Reset follows the same rule for school names.
OAuth remains adapter-owned. Explicit health checks use a model session through the
resolved Pi instance with visible output (confirmed by Chakrit during implementation);
configuration presence alone never counts as successful tool use.

## Implementation and verification

1. Validate the installation parser and supported version boundary with isolated fixtures;
   record the accepted backend contract, then implement shared context and preflight.
2. Add Pi selection, argument translation, provisioning, and MCP operations using existing
   owners; update help and specs together.
3. Earn behavioral test failures for missing installation and repair hints, mutation order,
   school-driven backend changes, custom environments, layered inventory/removal, and
   malformed configuration; cover launch/resume, prompt `@`/dash/path interpretation,
   skill coexistence, and existing backends through shared changes.
4. Run targeted/full tests, formatting and Clippy without increasing budgets; verify real
   Pi loading separately from recording fixtures, then audit the complete change.
   Agree on provider/model and cost before live model tests.
