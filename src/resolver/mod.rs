//! Config resolution: merge layered `Tree` + overrides into a `Resolved` view
//! with per-field provenance. Pure logic; no I/O, no binding lookups.
//!
//! See `docs/decisions/2026-04-27-config-resolution-redesign.md`.

mod imports;
mod merge;
mod project;
mod resolved;
mod source;

#[allow(unused_imports)]
pub use imports::{
    DiscoveryBySource, ImportCollision, ImportVerdict, ImportsResolution, ResolvedImport,
    UnknownImportPattern, resolve_imports,
};
pub use merge::merge;
pub use project::{Collision, Decision, Entry, UnknownPattern, resolve_skills};
pub use resolved::Resolved;
pub use source::{Source, Sourced};
