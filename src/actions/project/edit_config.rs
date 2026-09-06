//! Targeted edits retain the original TOML document; publication owns disk changes.

use std::path::Path;

use toml_edit::{DocumentMut, InlineTable, Item, Table, TableLike, Value};

use crate::ace::Ace;
use crate::config::ConfigError;
use crate::config::ace_toml::AceToml;

use super::publish_config::PublishConfig;

pub struct FieldEdit {
    tables: Vec<String>,
    key: String,
    edit: Edit,
}

enum Edit {
    Set(Value),
    Remove,
}

impl FieldEdit {
    pub fn new(key: impl Into<String>, value: impl Into<Value>) -> Self {
        Self {
            tables: Vec::new(),
            key: key.into(),
            edit: Edit::Set(value.into()),
        }
    }

    pub fn remove(key: impl Into<String>) -> Self {
        Self {
            tables: Vec::new(),
            key: key.into(),
            edit: Edit::Remove,
        }
    }

    pub fn in_tables(mut self, tables: impl IntoIterator<Item = String>) -> Self {
        self.tables = tables.into_iter().collect();
        self
    }

    pub fn strings(key: &str, values: &[String]) -> Self {
        Self::new(key, values.iter().collect::<Value>())
    }

    fn apply(&self, document: &mut DocumentMut) -> Result<(), ConfigError> {
        let table = descend(document.as_item_mut(), &self.tables)?;
        let Edit::Set(value) = &self.edit else {
            table.remove(&self.key);
            return Ok(());
        };
        let mut value = value.clone();
        if let Some(existing) = table.get(&self.key).and_then(Item::as_value) {
            *value.decor_mut() = existing.decor().clone();
        }
        match table.get_mut(&self.key) {
            Some(existing) => *existing = Item::Value(value),
            None => {
                table.insert(&self.key, Item::Value(value));
            }
        }
        Ok(())
    }
}

pub struct EditConfig<'a> {
    pub path: &'a Path,
    pub assignments: Vec<FieldEdit>,
}

impl EditConfig<'_> {
    pub fn run(&self, ace: &mut Ace) -> Result<(), ConfigError> {
        let original = match std::fs::read_to_string(self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let _: AceToml = toml::from_str(&original)?;
        let mut document = original.parse::<DocumentMut>()?;
        for assignment in &self.assignments {
            assignment.apply(&mut document)?;
        }
        let content = document.to_string();
        let _: AceToml = toml::from_str(&content)?;

        PublishConfig {
            path: self.path,
            content: &content,
        }
        .run(ace)?;
        Ok(())
    }
}

fn descend<'a>(
    item: &'a mut Item,
    parents: &[String],
) -> Result<&'a mut dyn TableLike, ConfigError> {
    let inline = item.is_inline_table();
    let table = item
        .as_table_like_mut()
        .ok_or_else(|| ConfigError::InvalidEdit("expected a TOML table".to_string()))?;
    let Some((name, rest)) = parents.split_first() else {
        return Ok(table);
    };
    let empty = if inline {
        Item::Value(Value::InlineTable(InlineTable::new()))
    } else {
        Item::Table(Table::new())
    };
    let child = table.entry(name).or_insert(empty);

    descend(child, rest)
}
