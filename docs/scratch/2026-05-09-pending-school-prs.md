# Pending school proposals (drafts not yet PR'd)

Saved from session memory before repo move. Two open PRs already exist on
prod9/school (#32 deny-warnings + clippy gate, #33 ace-audit skill); items below
are still drafts.

## 1. rust-coding skill — Rust 2024 let-chains

Add to `prod9/school:skills/rust-coding/`. Genuinely novel post-training-cutoff
syntax (Rust 2024 edition).

Use `if let ... && ...` chains to collapse nested `if let` blocks instead of
leaving `collapsible_if` clippy warnings:

```rust
// Old:
if let Some(p) = path {
    if has_traversal(p) {
        return Err(...);
    }
}

// Rust 2024:
if let Some(p) = path
    && has_traversal(p)
{
    return Err(...);
}
```

Multi-binding chains:

```rust
if let Ok(obj) = serde_json::from_str::<Value>(s)
    && let Some(statuses) = obj.get("statuses")
{
    return parse(statuses);
}
```

**Why:** clippy `collapsible_if` in Rust 1.88+/2024 now suggests let-chains as
the fix; nested `if let` produces lint noise under `#![deny(warnings)]`.

**How to apply:** when writing Rust 2024 code (check `edition` in Cargo.toml),
prefer let-chains over nested `if let`. Mandatory under `#![deny(warnings)]`.

## 2. rust-coding skill — enum→`label()` convention (PROD9-135)

Filed as PROD9-135. Document the convention that enum variants exposing a
human-readable string should provide an inherent `label(&self) -> &'static str`
method rather than free `*_name()` functions in callers.

Worked example: `Scope::label` (commit d8599ee) replaced
`cmd/skills::scope_name`. Canonical duplication case lives in
`list_skills.rs` / `explain_skill.rs` / `cmd/explain.rs` for `Status`, `Tier`,
`Decision` matches.

Land via school PR; coordinate with a codebase sweep so the convention is
documented before applying it broadly.

## 3. Training-drift note (NOT for the skill — for us)

During 2026-04-14 clippy cleanup, fixed 30 errors. Only let-chains (above) was
post-cutoff novel. The other 29 were basic long-standing lints
(`needless_borrows_for_generic_args` 13×, `let_and_return`,
`match_like_matches_macro`, `needless_lifetimes`, `useless_format`,
`unnecessary_lazy_evaluations`, `single_component_path_imports`,
`empty_line_after_doc_comments`, `doc_lazy_continuation`, `type_complexity`,
`never_loop`).

All should have been caught at write-time. Drift happened because prior agents
treated `cargo build` clean as done. `#![deny(warnings)]` only covers rustc
warnings, not clippy.

**Mitigation (school PR #32 covers this):** `cargo clippy --all-targets -- -D
warnings` as part of the done-gate alongside `cargo test`. CLAUDE.md Testing
section should reflect once #32 lands.
