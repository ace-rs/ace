This project has unconsolidated skills in
`{{ backend_dir }}/previous-skills/`. Before starting work:
1. Review previous skills and current school skills (symlinked in
   `{{ backend_dir }}/skills/`)
2. For each previous skill:
   - If a matching school skill exists: merge the content into it (edit
     through symlink)
   - If no matching school skill exists: move the folder from
     `previous-skills/` into the school's `skills/` directory
3. Delete `{{ backend_dir }}/previous-skills/` when all skills are
   consolidated
