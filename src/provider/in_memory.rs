use crate::provider::{Provider, ResolvedSecret};
use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;

pub struct InMemoryProvider {
    secrets: HashMap<String, String>,
}

impl InMemoryProvider {
    pub fn new() -> Self {
        InMemoryProvider {
            secrets: HashMap::new(),
        }
    }

    pub fn from_secrets(secrets: HashMap<String, String>) -> Self {
        InMemoryProvider { secrets }
    }

    pub fn insert_secret(&mut self, id: String, secret: String) {
        self.secrets.insert(id, secret);
    }
}

#[async_trait(?Send)]
impl Provider for InMemoryProvider {
    async fn provide_secrets(&self, ids: Vec<String>) -> Result<Vec<ResolvedSecret>> {
        let mut result = Vec::new();

        for id in ids {
            if let Some(secret) = self.secrets.get(&id) {
                result.push(ResolvedSecret {
                    id: id.clone(),
                    secret: secret.clone(),
                });
            }
        }

        Ok(result)
    }
}
