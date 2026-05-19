## Excluded skills

The following skills exist in the school but are filtered out by this project's
`ace.toml` (`include_skills` / `exclude_skills`):

{{ names }}

If the user asks to load one of these, do not say "no such skill" — explain that
the skill exists but is excluded, and that they can enable it by editing
`ace.toml` (`ace skills include <name>` or removing it from `exclude_skills`).
