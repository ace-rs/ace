# Backend Install

When the resolved backend binary is not found on `$PATH`, ACE offers to install it.

## Trigger

Bare `ace` run only. After Prepare, before Exec. Not during `ace setup`.

Covers two cases:
- **First run** — user clones a repo with `ace.toml`, runs `ace`, backend not installed.
- **Backend change** — school updates recommended backend, user runs `ace`, new backend missing.

## Detection

```rust
which("{binary}") // or Command::new("which").arg(binary).status()
```

If the binary is found on `$PATH`, skip to readiness check. If not found, prompt.

## Prompt

```
Claude Code is not installed. Install it? [Y/n]
```

- **Yes** — download and install the binary.
- **No** — abort with exit code.

## Install Method

Direct binary download per backend. No package managers, no npm.

### Platform Detection

Detect OS and architecture from `uname`:

| `uname -s` | OS        |
|-------------|-----------|
| `Darwin`    | `darwin`  |
| `Linux`     | `linux`   |

| `uname -m`         | Arch    |
|---------------------|---------|
| `x86_64` / `amd64` | `x64`   |
| `arm64` / `aarch64` | `arm64` |

### Per-Backend Download

**Claude Code**

1. Fetch version: `GET https://storage.googleapis.com/claude-code-dist-86c565f3-f756-42ad-8dfa-d59b1c096819/claude-code-releases/latest` → plain text version string.
2. Download binary: `GET {bucket}/{version}/{os}-{arch}/claude` → raw executable.
3. No extraction needed.

**Codex**

1. Download: `GET https://github.com/openai/codex/releases/latest/download/codex-{arch}-{target}.tar.gz`
   - Target examples: `aarch64-apple-darwin`, `x86_64-unknown-linux-gnu`
2. Extract `codex` binary from tar.gz.

### Install Target

`~/.local/bin/` — XDG convention.

Create the directory if it doesn't exist.

### PATH Check

After placing the binary, verify it's reachable:

```rust
Command::new("which").arg(binary).status()
```

If `which` fails, `~/.local/bin` is not in `$PATH`. Print:

```
Installed {binary} to ~/.local/bin/{binary}
~/.local/bin is not in your PATH. Add it:

  export PATH="$HOME/.local/bin:$PATH"
```

Then abort — don't exec a backend the shell can't find.

If `which` succeeds, continue to readiness check and Exec.

## Error Cases

- **Unsupported OS/arch** — error: `unsupported platform: {os}-{arch}`
- **Download fails** — error with HTTP status. Do not retry automatically.
- **No network** — same as download failure.
- **Binary already exists at target** — overwrite (user consented to install).

## Not in Scope

- Windows support.
- Auto-update of backends (backends handle their own updates).
- Install during `ace setup`.
- Uninstall.
