use std::collections::{HashMap, HashSet};

use itertools::Itertools;

use crate::error::{AwsPsSdkError, AwsSmApiError, AwsSmSdkError, Error};

/// A type that implements `Provider` allows provision of secret configurations
pub trait Provider {
    type Config;

    async fn try_provide_secrets(
        &self,
        ids: Vec<Self::Config>,
    ) -> Result<Vec<Option<String>>, Error>;
}

/// Fetches secrets from AWS Secrets Manager
pub struct SecretsManagerProvider {
    client: aws_sdk_secretsmanager::Client,
}

impl SecretsManagerProvider {
    pub async fn new() -> Self {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_secretsmanager::Client::new(&config);

        Self { client }
    }
}

impl Provider for SecretsManagerProvider {
    type Config = String;

    // All the expects are because the AWS SDK isn't idiomatic
    async fn try_provide_secrets(
        &self,
        ids: Vec<Self::Config>,
    ) -> Result<Vec<Option<String>>, Error> {
        // Create a deduped vector of secret IDs to fetch from AWS
        let unique_ids: Vec<String> = ids
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();

        let mut key_map: HashMap<Self::Config, String> = HashMap::new();

        for chunk in &unique_ids.into_iter().chunks(20) {
            let secrets = self
                .client
                .batch_get_secret_value()
                .set_secret_id_list(Some(chunk.collect()))
                .send()
                .await
                .map_err(AwsSmSdkError::from)?;

            if let Some(error) = secrets.errors.and_then(|errors| {
                errors
                    .into_iter()
                    .filter(|e| e.error_code() != Some("ResourceNotFoundException"))
                    .next()
            }) {
                return Err(AwsSmApiError::from(error).into());
            };

            key_map.extend(
                secrets
                    .secret_values
                    .expect("should have secrets if there were no ResourceNotFound errors")
                    .into_iter()
                    .map(|s| {
                        (
                            s.name.expect("should have a name"),
                            s.secret_string.expect("should have a secret string"),
                        )
                    }),
            );
        }

        Ok(ids.into_iter().map(|s| key_map.get(&s).cloned()).collect())
    }
}

pub struct ParameterStoreProvider {
    client: aws_sdk_ssm::Client,
}

impl ParameterStoreProvider {
    pub async fn new() -> Self {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_ssm::Client::new(&config);

        Self { client }
    }
}

impl Provider for ParameterStoreProvider {
    type Config = String;

    async fn try_provide_secrets(&self, ids: Vec<String>) -> Result<Vec<Option<String>>, Error> {
        let deduped = ids.clone().into_iter().collect::<HashSet<_>>().into_iter();

        let mut key_map = HashMap::new();

        for chunk in &deduped.chunks(10) {
            let resp = self
                .client
                .get_parameters()
                .set_with_decryption(Some(true))
                .set_names(Some(chunk.collect()))
                .send()
                .await
                .map_err(|err| AwsPsSdkError::from(err))?;

            for param in resp
                .parameters
                .expect("should have parameters at this point")
            {
                key_map.insert(
                    param.name.expect("should have name"),
                    param.value.expect("should have value"),
                );
            }
        }

        Ok(ids.into_iter().map(|s| key_map.get(&s).cloned()).collect())
    }
}
