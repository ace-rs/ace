# Prompt Templating

Covers session prompt composition, template rendering, and delivery to the backend.

## Session Prompt

The session prompt is the instruction text ACE passes to the backend at launch. It is distinct
from the backend's own system prompt.

### Composition

Built by concatenating layers, separated by blank lines:

1. **Built-in** — `prompt_session.md`, always present. Contains school name/concept, symlink
   edit flow, config file awareness, debugging tips. Rendered with `{{ school_name }}`.
2. **Role** — resolved from the user's selected role in the school's `[[roles]]` list. Injected
   as `Role: {name}\n{prompt}`. Only present when a role is set and found in the school. See
   [roles.md](roles.md).
3. **School** — `session_prompt` field in `school.toml`. Domain-specific instructions from the
   school maintainer. Injected verbatim (no template substitution).
4. **Project** — `session_prompt` field in `ace.toml` (or user-level `~/.config/ace/ace.toml`).
   Resolved across config layers (user < project < local, last wins).
   Injected verbatim (no template substitution).
5. **Skill change summary** — `prompt_changes.md`, only when skills changed since last session.
   Lists added, updated, and removed skills. Rendered with `{{ changes }}`.
6. **School changes** — `prompt_school_changes.md`, only when a school clone exists (remote
   schools). Contains proposal workflow steps. Rendered with `{{ school_clone }}`. When the
   clone has uncommitted changes, `prompt_dirty_school.md` is appended (no placeholders).
7. **Previous skills** — `prompt_previous_skills.md`, only when a `previous-skills/` directory
   exists. Consolidation guidance. Rendered with `{{ backend_dir }}`. Note: post-2026-04-23
   ACE no longer creates `previous-skills/` (per-skill linking handles foreign entries
   in-place); this layer only fires for projects that adopted the legacy whole-dir layout.

Empty/absent layers are skipped.

### Template File Convention

Each `.md` template is a self-contained text block — no leading blank lines, file ends with a
single newline. Separation between blocks is the composition code's responsibility: parts are
trimmed, empties filtered, then joined with `"\n\n"`. This single rule handles all newline
management. Conditional content uses a separate `.md` file with a conditional `parts.push()`,
never an embedded placeholder that resolves to empty string.

### Config Fields

`school.toml`:

```toml
name = "acme"
session_prompt = "Follow Acme coding standards..."
```

`ace.toml`:

```toml
school = "acme/school"
session_prompt = "This project uses PostgreSQL..."
```

### Delivery

The composed prompt is passed to the backend using that backend's native prompt-delivery
mechanism. For some backends this is a `--system-prompt <prompt>` CLI flag; for others it is an
initial positional prompt.

## Template Engine

Lives in `src/templates/` — `mod.rs` (Template struct) and `parser.rs` (state machine).

`Template::parse(input)` parses `{{ key }}` placeholders in a single pass, zero-copy. The
parsed template supports `placeholders()` (returns unique names) and `substitute(values)`
(returns rendered string with missing keys resolved to empty string).

### Syntax

- `{{ name }}` — placeholder, resolved by `substitute()`.
- Whitespace inside braces is flexible: `{{name}}`, `{{ name }}`, `{{  name  }}` all match.
- Name must be `[a-zA-Z0-9_]+`.
- Newlines inside `{{ }}` abort the placeholder (treated as literal).
- Single braces `{name}` are not placeholders — passthrough.
- Invalid names (hyphens, spaces, etc.) are preserved as literal text.

### Current Placeholders

| Placeholder        | Used in                   | Source                        | Example                              |
|--------------------|---------------------------|-------------------------------|--------------------------------------|
| `{{ school_name }}`  | `prompt_session.md`, project/school CLAUDE.md templates | School display name | `Acme` |
| `{{ backend_dir }}`  | `prompt_previous_skills.md`, project CLAUDE.md template | Backend directory name        | `.claude` |
| `{{ school_clone }}` | `prompt_school_changes.md` | School clone path             | `/home/user/.local/share/ace/org/school` |
| `{{ changes }}`      | `prompt_changes.md`       | Formatted change list (built by `session.rs`) | `- Added: \`new-skill\`` |

### Adding a Placeholder

1. Use `{{ field_name }}` in any `.md` file under `src/templates/builtins/`.
2. Pass the value in the `HashMap` at the call site (in `session.rs` or the action that
   renders the template).

### Module Layout

```
src/templates/
  mod.rs          — Template struct (parse, substitute, placeholders)
  parser.rs       — Parser state machine
  builtins.rs     — include_str! constants for all .md files
  session.rs      — build_session_prompt() composition
  builtins/       — .md template files
```
