mod aws_parameter_store;
mod aws_secrets_manager;

use anyhow::Result;

pub use aws_parameter_store::AwsParameterStoreProvider;
pub use aws_secrets_manager::AwsSecretsManagerProvider;

pub struct ResolvedSecret<T> {
    config: T,
    secret: String,
}

/// A type that implements `Provider` allows provision of secret configurations
pub trait Provider {
    type Config;

    async fn provide_secrets(
        &self,
        configs: Vec<Self::Config>,
    ) -> Result<Vec<ResolvedSecret<Self::Config>>>;
}
