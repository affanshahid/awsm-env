use std::collections::HashMap;

use itertools::Itertools;

use crate::error::{AwsApiError, AwsSdkError, Error};

/// Fetches secrets from AWS Secrets Manager
// All the expects are because the AWS SDK isn't idiomatic
pub async fn fetch_secrets_from_aws(ids: Vec<String>) -> Result<Vec<String>, Error> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_secretsmanager::Client::new(&config);

    let mut key_map = HashMap::new();

    for chunk in &ids.clone().into_iter().chunks(20) {
        let secrets = client
            .batch_get_secret_value()
            .set_secret_id_list(Some(chunk.collect()))
            .send()
            .await
            .map_err(AwsSdkError::from)?;

        if let Some(error) = secrets.errors.and_then(|errors| errors.into_iter().next()) {
            return Err(AwsApiError::from(error).into());
        };

        key_map.extend(
            secrets
                .secret_values
                .expect("should have secrets if there were no errors")
                .into_iter()
                .map(|s| {
                    (
                        s.name.expect("should have a name"),
                        s.secret_string.expect("should have a secret string"),
                    )
                }),
        );
    }

    Ok(ids
        .into_iter()
        .map(|s| {
            key_map
                .remove(&s)
                .expect("should have a result at this point")
        })
        .collect())
}
