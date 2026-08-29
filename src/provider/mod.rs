mod aws_parameter_store;
mod aws_secrets_manager;
mod in_memory;

use anyhow::Result;

use async_trait::async_trait;
pub use aws_parameter_store::AwsParameterStoreProvider;
pub use aws_secrets_manager::AwsSecretsManagerProvider;
pub use in_memory::InMemoryProvider;

pub struct ResolvedSecret {
    pub id: String,
    pub secret: String,
}

/// A type that implements `Provider` allows provision of secret configurations
#[async_trait(?Send)]
pub trait Provider {
    #[allow(async_fn_in_trait)]
    async fn provide_secrets(&self, ids: Vec<String>) -> Result<Vec<ResolvedSecret>>;
}
