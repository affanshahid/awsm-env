use crate::provider::{Provider, ResolvedSecret};
use anyhow::{Result, anyhow};
use itertools::Itertools;

/// Fetches secrets from AWS Secrets Manager
pub struct AwsSecretsManagerProvider {
    client: aws_sdk_secretsmanager::Client,
}

impl AwsSecretsManagerProvider {
    pub async fn new() -> Self {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_secretsmanager::Client::new(&config);

        Self { client }
    }
}

impl Provider for AwsSecretsManagerProvider {
    // All the expects are because the AWS SDK isn't idiomatic
    async fn provide_secrets(&self, ids: Vec<String>) -> Result<Vec<ResolvedSecret<String>>> {
        let mut result = Vec::new();

        for chunk in &ids.into_iter().chunks(20) {
            let secrets = self
                .client
                .batch_get_secret_value()
                .set_secret_id_list(Some(chunk.collect()))
                .send()
                .await?;

            let first_error = secrets.errors.and_then(|errors| {
                errors
                    .into_iter()
                    .filter(|e| e.error_code() != Some("ResourceNotFoundException"))
                    .next()
            });

            if let Some(error) = first_error {
                return Err(anyhow!("Failed to fetch secrets: {:?}", error));
            }

            result.extend(
                secrets
                    .secret_values
                    .expect("should have secrets if there were no ResourceNotFound errors")
                    .into_iter()
                    .map(|s| ResolvedSecret {
                        config: s.name.expect("should have a name"),
                        secret: s.secret_string.expect("should have a secret string"),
                    }),
            );
        }

        Ok(result)
    }
}
