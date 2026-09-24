# Authentication

## MCP Server Authentication

ACE delegates MCP authentication entirely to the backend. Supported backends
(Claude Code, Codex) handle OAuth discovery, token acquisition, storage, and refresh
for remote MCP servers. See [mcp.md](mcp.md) for full details on the remote-only MCP design.

| Backend  | Auth behavior                              | Token storage                             |
|----------|--------------------------------------------|-------------------------------------------|
| Claude   | Auto-prompts on 401                        | System keychain                           |
| Codex    | Managed in-session via `/mcp`              | `~/.codex/auth.json` or OS keyring        |
| OpenCode | Auto-prompts on 401                        | `~/.local/share/opencode/auth.json`       |

ACE does not implement OAuth, store tokens, or manage token refresh.

## School Repository Authentication

ACE uses Git's existing SSH identities and HTTPS credential helpers. For a new GitHub
school clone, it checks repository access over SSH, then HTTPS, and uses the successful
transport. Account authentication alone does not establish access to a private school.
Cloning carries the project's SSH command and credential-helper settings into Git's
process configuration, because Git otherwise changes configuration context when it
creates the destination repository. These settings are not persisted into the cache;
later pulls use the cached repository's and user's own Git configuration.

When both checks fail during `ace setup`, ACE retains their diagnostics and offers an
attended HTTPS clone. Git receives the terminal directly. ACE explains that GitHub
requires a personal access token in Git's password field and links to token creation;
the token needs access to the requested repository, including any organization approval.
No GitHub CLI or additional credential helper is required.

The attended clone itself establishes access, avoiding a separate credential prompt
for an authenticated probe. Existing credential and askpass helpers remain available.
Without a browser-capable helper, users create a token on GitHub and enter it into Git;
ACE does not provide its own browser/device login.

ACE does not collect, log, or store credentials, install helpers, or change global Git
credential configuration. Existing helpers control persistence; without one, later
private-repository operations can require credentials again. Completing setup does not
guarantee that credentials have been saved.

Credential entry requires terminal input, ACE's normal prompt policy, and no explicit
`GIT_TERMINAL_PROMPT` prohibition (such as `0`, `false`, or `off`). CI, `--yes`, and
porcelain runs do not enter this path. They report how to retry in a terminal or
configure Git credentials externally.
When `GIT_TERMINAL_PROMPT` is present, ACE permits credential entry only for affirmative
boolean values or nonzero integers; unrecognized values keep the attended path disabled.
Cancellation stops setup; credential failure does not trigger a retry loop.

Background access probes suppress native prompts and known Git Credential Manager UI
through their documented controls. Arbitrary user-installed credential programs retain
their own behavior; bounded probes stop waiting if a program fails to return.

References: [Git credentials](https://git-scm.com/docs/gitcredentials) and
[GitHub token creation](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens).
