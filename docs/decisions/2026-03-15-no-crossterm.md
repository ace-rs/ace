# 001: No crossterm — use inquire + indicatif for terminal interaction

**Status:** Accepted

## Context

ACE only needs simple prompts/questions and progress indicators, not full terminal manipulation.

## Decision

Use `inquire` for interactive prompts and `indicatif` for progress bars. No full terminal lib (crossterm, termion, etc.).

## Rationale

- Simpler API surface for ACE's interaction patterns (select, text input, spinners).
- Lower compilation cost and faster builds.
- crossterm is overkill — ACE never needs raw mode, cursor control, or screen management.

## Note

The `ace fly` easter egg uses raw ANSI escapes directly for its one-off alt-screen animation rather than pulling in crossterm as a dependency.
