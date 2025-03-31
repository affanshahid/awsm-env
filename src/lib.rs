//! Awsm Env
//!
//! A lightweight utility for syncing AWS Secrets Manager secrets to environment variables.

mod error;
mod formatters;
mod parser;
mod providers;

use std::borrow::Cow;

use error::Error;
pub use formatters::{EnvFormatter, Formatter, JsonFormatter, ShellFormatter};
use indexmap::IndexMap;
pub use parser::{EnvEntries, EnvEntry, parse};
pub use providers::fetch_secrets_from_aws;

pub async fn process_entries<'a>(
    mut entries: EnvEntries<'a>,
    overrides: &'a IndexMap<String, String>,
) -> Result<IndexMap<&'a str, Cow<'a, str>>, Error> {
    let secrets = fetch_secrets_from_aws(
        entries
            .iter()
            .filter_map(|e| e.secret_id.as_ref().map(|s| s.to_string()))
            .collect(),
    )
    .await?;

    entries
        .iter_mut()
        .filter(|e| e.secret_id.is_some())
        .zip(secrets.into_iter())
        .for_each(|(e, s)| e.value = Some(Cow::Owned(s)));

    let mut result: IndexMap<&str, Cow<str>> = entries
        .into_iter()
        .filter_map(|e| e.value.map(|v| (e.key, v)))
        .collect();

    result.extend(
        overrides
            .iter()
            .map(|(key, value)| (key.as_str(), Cow::Borrowed(value.as_str()))),
    );

    Ok(result)
}
