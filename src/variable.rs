use std::{ops::Deref, slice::IterMut};

use indexmap::IndexMap;

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderConfig {
    AwsSecretsManager(String),
    AwsParameterStore(String),
}

impl ProviderConfig {
    pub fn id(&self) -> &str {
        match self {
            ProviderConfig::AwsSecretsManager(id) => id,
            ProviderConfig::AwsParameterStore(id) => id,
        }
    }
}

/// Represents a single environment variable binding
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Variable {
    pub key: String,
    pub required: bool,
    pub default: Option<String>,
    pub value: Option<String>,
    pub provider_config: Option<ProviderConfig>,
}

impl Variable {
    pub fn promote_default(&mut self) {
        self.value = self.default.take();
    }

    pub fn demote_value(&mut self) {
        self.default = self.value.take();
    }

    pub fn drop_default(&mut self) {
        self.default = None;
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_none() && self.default.is_none()
    }

    pub fn merge(&mut self, mut other: Variable) {
        if self.key != other.key {
            panic!("Cannot merge variables with different keys");
        }

        if other.required {
            self.required = true;
        }

        if other.default.is_some() {
            self.default = other.default.take();
        }

        if other.value.is_some() {
            self.value = other.value.take();
        }

        if other.provider_config.is_some() {
            self.provider_config = other.provider_config.take();
        }
    }
}

/// List of [`Variable`]s.
#[derive(Debug, PartialEq)]
pub struct Variables(Vec<Variable>);

impl Variables {
    pub fn new() -> Self {
        Variables(Vec::new())
    }

    pub fn find_by_key(&self, key: &str) -> Option<&Variable> {
        self.0.iter().find(|v| v.key == key)
    }

    pub fn insert(&mut self, variable: Variable) {
        let existing = self.0.iter().position(|v| v.key == variable.key);

        if let Some(idx) = existing {
            self.0.remove(idx);
        }

        self.0.push(variable);
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, Variable> {
        self.0.iter_mut()
    }

    pub fn drop_empty(&mut self) {
        self.0.retain(|v| !v.is_empty());
    }

    pub fn merge(&mut self, other: Variables) {
        for other_var in other {
            let Some(var) = self.0.iter_mut().find(|v| v.key == other_var.key) else {
                self.insert(other_var);
                continue;
            };

            var.merge(other_var);
        }
    }
}

impl Into<IndexMap<String, String>> for Variables {
    fn into(self) -> IndexMap<String, String> {
        self.into_iter()
            .filter_map(|var| var.value.or(var.default).map(|val| (var.key, val)))
            .collect()
    }
}

impl From<IndexMap<String, String>> for Variables {
    fn from(value: IndexMap<String, String>) -> Self {
        Variables(
            value
                .into_iter()
                .map(|(key, val)| Variable {
                    key,
                    value: Some(val),
                    ..Default::default()
                })
                .collect(),
        )
    }
}

impl From<Vec<Variable>> for Variables {
    fn from(value: Vec<Variable>) -> Self {
        Variables(value)
    }
}

impl Deref for Variables {
    type Target = Vec<Variable>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl IntoIterator for Variables {
    type Item = Variable;
    type IntoIter = std::vec::IntoIter<Variable>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
