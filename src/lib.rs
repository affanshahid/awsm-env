//! Awsm Env
//!
//! A lightweight utility for syncing AWS Secrets Manager secrets to environment variables.

mod error;
mod formatters;
mod parser;
mod providers;

use std::borrow::Cow;

use error::Error;
use formatters::OutputEntry;
pub use formatters::{EnvFormatter, Formatter, JsonFormatter, ShellFormatter};
pub use parser::{EnvEntries, EnvEntry, parse};
pub use providers::fetch_secrets_from_aws;

pub async fn process_entries<'a>(
    entries: &'a mut EnvEntries<'a>,
) -> Result<Vec<OutputEntry<'a>>, Error> {
    let secrets = fetch_secrets_from_aws(
        entries
            .iter()
            .filter_map(|e| e.secret_id.as_ref().map(|s| s.to_string())),
    )
    .await?;

    entries
        .iter_mut()
        .filter(|e| e.secret_id.is_some())
        .zip(secrets.into_iter())
        .for_each(|(e, s)| e.value = Some(Cow::Owned(s)));

    Ok(entries
        .iter()
        .filter_map(|e| e.value.as_ref().map(|v| OutputEntry(e.key, &v)))
        .collect())
}
