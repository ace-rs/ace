//! Config resolution: merge layered `Tree` + overrides into a `Resolved` view
//! with per-field provenance. Pure logic; no I/O, no binding lookups.
//!
//! See `docs/spec/architecture.md`.

mod merge;
mod resolved;
mod source;

pub use merge::merge;
pub use resolved::Resolved;
pub use source::{Source, Sourced};
