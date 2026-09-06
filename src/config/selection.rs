//! Discovery-independent selection policy shared by runtime and config inspection.

use std::collections::HashSet;

use super::ace_toml::AceToml;
use super::resolve::{Source, Sourced};
use super::tree::Tree;

pub struct Policy<'a> {
    layers: [(Source, Option<&'a AceToml>); 3],
}

impl<'a> Policy<'a> {
    pub fn from_tree(tree: &'a Tree) -> Self {
        Self {
            layers: [
                (Source::User, tree.user.as_ref()),
                (Source::Project, tree.project.as_ref()),
                (Source::Local, tree.local.as_ref()),
            ],
        }
    }

    pub fn from_layers(user: &'a AceToml, project: &'a AceToml, local: &'a AceToml) -> Self {
        Self {
            layers: [
                (Source::User, Some(user)),
                (Source::Project, Some(project)),
                (Source::Local, Some(local)),
            ],
        }
    }

    /// An empty default base means all discovered skills, without requiring discovery.
    pub fn skills(&self) -> Sourced<&'a [String]> {
        let winner = self.layers.iter().rev().find_map(|(source, layer)| {
            let config = layer.as_ref()?;
            (!config.skills.is_empty()).then_some(Sourced::new(config.skills.as_slice(), *source))
        });

        winner.unwrap_or_else(|| Sourced::at_default(&[]))
    }

    pub fn include_skills(&self) -> Union<'a> {
        self.union(|config| &config.include_skills)
    }

    pub fn exclude_skills(&self) -> Union<'a> {
        self.union(|config| &config.exclude_skills)
    }

    pub fn exclude_mcp(&self) -> Union<'a> {
        self.union(|config| &config.exclude_mcp)
    }

    fn union(&self, pick: fn(&AceToml) -> &[String]) -> Union<'a> {
        Union {
            sources: self
                .layers
                .map(|(source, layer)| (source, layer.map(pick).unwrap_or(&[]))),
        }
    }
}

pub struct Union<'a> {
    sources: [(Source, &'a [String]); 3],
}

impl<'a> Union<'a> {
    /// Keep per-source occurrences for skill traces and same-scope collision diagnostics.
    pub fn sources(&self) -> [(Source, &'a [String]); 3] {
        self.sources
    }

    /// Values follow their first occurrence in user → project → local order.
    pub fn values(&self) -> Vec<String> {
        let mut seen = HashSet::new();

        self.sources
            .iter()
            .flat_map(|(_, patterns)| *patterns)
            .filter(|pattern| seen.insert(pattern.as_str()))
            .cloned()
            .collect()
    }
}
