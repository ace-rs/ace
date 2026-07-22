# `ace template` subcommand

Status: design note, not yet implemented.

A debugging / inspection affordance for ACE's built-in prompt templates: render
them as ACE actually does, with default or caller-supplied variables, and print
to stdout. Useful for inspecting what ACE injects into the backend (the session
prompt, the learn prompt, the school-changes prompt, etc.) and for copying that
output into another tool for testing or comparison.

Alias: `ace tmpl`.

## Invocation modes

| Invocation                                | Behavior                                                  |
|-------------------------------------------|-----------------------------------------------------------|
| `ace template <name>`                     | Render builtin `<name>` with default vars.                |
| `ace template <name> k=v [k2=v2 ...]`     | Render builtin `<name>`, overriding/adding vars from CLI. |
| `ace template k=v [k2=v2 ...] < input`    | Read template from stdin, render with given vars.         |
| `ace template --help`                     | List available builtin templates.                         |

The `<name>` argument is a builtin template key — one of the names registered
in `src/templates/builtins.rs` (e.g. `session`, `learn`, `school_changes`,
`excluded_skills`, `school_instructions`, `school_readme`,
`project_claude_md`). The name set is finite and discoverable.

## Default vars

Each builtin has a known placeholder set (`Template::placeholders()`). When
rendering without `k=v` overrides, every placeholder is populated with a
deterministic stub value derived from its name (e.g. `school_name` →
`"<school_name>"`). This produces a readable rendering even without context,
and the stub form makes it obvious which substitutions are placeholder fills
vs. real content.

`k=v` overrides take precedence over stubs. Unknown keys are silently allowed
(they have no placeholder to fill).

## Stdin mode

When no `<name>` is supplied and stdin is not a terminal, treat stdin as the
template source. All non-flag arguments are parsed as `k=v` pairs. The same
substitution rules apply as for builtin mode.

If stdin is a terminal and no `<name>` is given, error out and suggest
`--help`.

## Idempotent canonical re-emit

`ace template <name>` re-emits the builtin template after the parser has
trimmed placeholder whitespace. Result: regardless of how a builtin file
stores its placeholders (`{{name}}`, `{{ name }}`, `{{  name  }}`), the
rendered output is canonical. This doubles as a low-tech fmt for templates
during internal dev — `cargo run -- template <name> > expected.md` produces
the canonical rendering.

## Out of scope

- Writing back to disk. The subcommand is read-only.
- Rendering external (non-builtin) template files by path. If the use case
  emerges, add `--file <path>` later.
- School context. Templates are crate-internal `include_str!`'d strings;
  rendering does not require an `Ace` or a school.

## Implementation sketch

- New module `src/cmd/template.rs`, dispatched from `src/cmd/mod.rs` clap.
- Use existing `Template::parse` / `Template::placeholders` /
  `Template::substitute` from `src/templates/mod.rs`.
- Builtin name registry: a static `&[(&str, &str)]` mapping name → template
  source, derived from `src/templates/builtins.rs` constants.
- `k=v` parser: simple `split_once('=')` with whitespace trim; error on bad
  form.
- Stdin: read once via `io::stdin().read_to_string`. Pipe detection via
  `io::IsTerminal`.

## Open design questions

- Should builtin names use the constant case (`SCHOOL_INSTRUCTIONS_MD`),
  snake_case (`school_instructions`), or kebab-case (`school-instructions`)?
  Recommend kebab-case — matches subcommand UX conventions.
- Should `--help` list templates inline, or should there be a dedicated
  `ace template list`? Recommend inline — keeps surface area small.
- Should the subcommand also expose project/session prompt outputs (which
  require `Ace` context)? Probably yes, but design that as a follow-up; the
  initial cut should be context-free builtins only.
