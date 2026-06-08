//! Skill resolution — turns raw config/declaration patterns into per-identity
//! verdicts with provenance, carrying [`Locator`](crate::skills::Locator)
//! natively rather than round-tripping through strings.
//!
//! Two sibling resolvers with distinct scope taxonomies (see
//! `docs/decisions/2026-06-05-resolver-dissolution.md`):
//!   - [`project`] — `ace.toml`'s user / project / local scopes, stamping each
//!     `Skill<Validated>` into a `Skill<Decided>`.
//!   - [`imports`] — `school.toml`'s `[[imports]]` declarations, merged across
//!     decls with first-wins-and-warn on identity collisions.

mod imports;
mod project;

pub use imports::{Discovery, ImportVerdict, ImportsResolution, resolve_imports};
pub use project::{Collision, Decision, Entry, InvalidPattern, UnknownPattern};
