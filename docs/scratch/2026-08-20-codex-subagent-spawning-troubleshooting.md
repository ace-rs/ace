<!-- not spec/decision because: this is an exploratory troubleshooting record to ingest later. -->

# Codex subagent spawning: incident and resolution

## Problem

The Codex session could not spawn a `gpt-5.6-sol` subagent. The assistant initially reported that no native delegation tool was available, despite subagents having worked in an earlier session.

## Research

- OpenAI documents multi-agent orchestration as a beta capability, but availability depends on the active Codex/API surface and session configuration: <https://developers.openai.com/api/docs/guides/latest-model>
- Codex issue [#24900](https://github.com/openai/codex/issues/24900) reports a desktop surface that lacked `spawn_agent` while the CLI could spawn subagents.
- Codex issue [#24069](https://github.com/openai/codex/issues/24069) documents a native subagent regression caused by feature/provider/version differences.
- Community reports identified model-generation compatibility: Sol is Multi-Agent V2 while Luna was catalogued as Multi-Agent V1. Cross-version spawning can therefore fail.

Local checks showed:

```text
codex-cli 0.148.0
multi_agent       true
multi_agent_v2    false   (before the fix)
gpt-5.6-sol       v2
gpt-5.6-luna      v1
```

## Solution

Enable the V2 multi-agent feature in `~/.codex/config.toml`:

```toml
[features]
multi_agent_v2 = true
```

After the change, verification showed:

```text
multi_agent       true
multi_agent_v2    true
gpt-5.6-sol       v2
gpt-5.6-luna      v1
```

The parent session remained Luna; it was not switched to Sol and no higher reasoning setting was required.

## Verification

Native delegation became available in the active session. A `gpt-5.6-sol` subagent was spawned with low reasoning effort and returned:

```text
Hello
```

This confirms that the parent can now delegate to a Sol worker without running the parent session as Sol.
