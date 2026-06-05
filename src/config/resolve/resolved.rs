use std::collections::HashMap;

use super::source::Sourced;
use crate::config::ace_toml::{BackendDecl, Trust};

/// Merged config view with provenance per field.
///
/// Built by `merge()` — pure transform over `Tree` + overrides. Infallible;
/// validation (e.g. backend name exists) happens later in the binding layer.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub school_specifier: Sourced<Option<String>>,
    pub backend_name: Sourced<String>,
    pub backend_decls: Vec<Sourced<BackendDecl>>,
    pub session_prompt: Sourced<String>,
    pub env: HashMap<String, Sourced<String>>,
    pub trust: Sourced<Trust>,
    pub resume: Sourced<bool>,
    pub skip_update: Sourced<bool>,
}
