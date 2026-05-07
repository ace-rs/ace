You are studying this project so ACE can narrow which skills load by default.

# Your tasks

1. **Study the project.** Read the source tree, configs, build files, existing
   instructions file (CLAUDE.md / AGENTS.md / etc.), and anything else that tells
   you what this project is, how it's built, and what conventions apply.

2. **Edit your own instructions file in place.** You know which file that is for
   the backend you're running under (CLAUDE.md for Claude, AGENTS.md for Codex,
   etc.). The edit must include:
   - **Project-study notes**: stack, layout, conventions, gotchas. Concise and
     factual — no filler.
   - **A "load these skills" section** pointing at the skill names you'll
     return below. This guides future sessions reading the file to pull the
     right skills.

3. **Output to stdout** the final desired set of skills for this project, as
   skill names or glob patterns, one per line. **Strict format** — no prose,
   no headers, no fences, no empty lines, no explanations. Just names and
   globs.

   Globs (e.g. `frontend-*`) are valid — `ace.toml`'s `skills` key supports
   them. Use a glob when a related cluster of skills all fit the project,
   instead of listing every member individually.

# Available skills

The skill names you may pick from (and the pool that glob patterns match
against):

{{ available_skills }}

# Output format

Strict. Example:

```
general-coding
rust-coding
simplify
markdown-writing
frontend-*
```

That's the entire stdout contract. The list you print **replaces** the
project's `skills` filter — it's the final desired set, not toggles.
