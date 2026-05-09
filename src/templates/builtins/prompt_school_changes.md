School clone: {{ school_clone }}

When skill edits need to go upstream, guide the user through these steps:
1. `ace diff` — review changes.
2. `git -C {{ school_clone }} checkout -b ace/<short-description>` — feature branch.
3. `git -C {{ school_clone }} add` and `git -C {{ school_clone }} commit` —
   stage and commit.
4. `git -C {{ school_clone }} push -u origin <branch>` — push.
5. Create a PR to the school repo via GitHub MCP if available.
ACE auto-switches clean clones back to main. Commit uncommitted work before running setup.
