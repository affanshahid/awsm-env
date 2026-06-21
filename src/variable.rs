use std::ops::Deref;

use crate::provider::{AwsParameterStoreProvider, AwsSecretsManagerProvider, Provider};

#[derive(Debug, PartialEq, Eq)]
pub enum ProviderConfig {
    AwsSecretsManager(<AwsSecretsManagerProvider as Provider>::Config),
    AwsParameterStore(<AwsParameterStoreProvider as Provider>::Config),
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

/// List of [`Variable`]s.
pub struct Variables(Vec<Variable>);

impl Variables {
    pub fn new() -> Self {
        Variables(Vec::new())
    }

    pub fn get_by_key(&self, key: &str) -> Option<&Variable> {
        self.0.iter().find(|v| v.key == key)
    }

    pub fn push(&mut self, variable: Variable) {
        let existing = self.0.iter().position(|v| v.key == variable.key);

        if let Some(idx) = existing {
            eprintln!("Warning: Duplicate declaration for {}", variable.key);
            self.0.remove(idx);
        }

        self.0.push(variable);
    }
}

impl Deref for Variables {
    type Target = Vec<Variable>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
