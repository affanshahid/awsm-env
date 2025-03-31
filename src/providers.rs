use itertools::Itertools;

use crate::error::{AwsApiError, AwsSdkError, Error};

/// Fetches secrets from AWS Secrets Manager
pub async fn fetch_secrets_from_aws(
    ids: impl IntoIterator<Item = String>,
) -> Result<Vec<String>, Error> {
    let config = aws_config::load_from_env().await;
    let client = aws_sdk_secretsmanager::Client::new(&config);

    let mut result = Vec::new();

    for chunk in &ids.into_iter().chunks(20) {
        let secrets = client
            .batch_get_secret_value()
            .set_secret_id_list(Some(chunk.collect()))
            .send()
            .await
            .map_err(AwsSdkError::from)?;

        if let Some(error) = secrets.errors.and_then(|errors| errors.into_iter().next()) {
            return Err(AwsApiError::from(error).into());
        };

        result.extend(
            secrets
                .secret_values
                .expect("should have secrets if there were no errors")
                .into_iter()
                .map(|s| s.secret_string.expect("should have a secret string")),
        );
    }

    Ok(result)
}
