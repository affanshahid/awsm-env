use crate::provider::{Provider, ResolvedSecret};

use anyhow::Result;
use itertools::Itertools;

pub struct AwsParameterStoreProvider {
    client: aws_sdk_ssm::Client,
}

impl AwsParameterStoreProvider {
    pub async fn new() -> Self {
        let config = aws_config::load_from_env().await;
        let client = aws_sdk_ssm::Client::new(&config);

        Self { client }
    }
}

impl Provider for AwsParameterStoreProvider {
    // All the expects are because the AWS SDK isn't idiomatic
    async fn provide_secrets(&self, ids: Vec<String>) -> Result<Vec<ResolvedSecret<String>>> {
        let mut result = Vec::new();

        for chunk in &ids.into_iter().chunks(10) {
            let resp = self
                .client
                .get_parameters()
                .set_with_decryption(Some(true))
                .set_names(Some(chunk.collect()))
                .send()
                .await?;

            result.extend(
                resp.parameters
                    .expect("should have parameters")
                    .into_iter()
                    .map(|p| ResolvedSecret {
                        config: p.name.expect("should have name"),
                        secret: p.value.expect("should have value"),
                    }),
            );
        }

        Ok(result)
    }
}
