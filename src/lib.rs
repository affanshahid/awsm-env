//! Awsm Env
//!
//! A lightweight utility for syncing AWS Secrets Manager secrets to environment variables.

mod error;
mod formatters;
mod parser;
mod providers;

use std::{borrow::Cow, collections::HashMap, sync::OnceLock};

use error::Error;
pub use formatters::{EnvFormatter, Formatter, JsonFormatter, ShellFormatter};
use indexmap::IndexMap;
pub use parser::{EnvEntries, EnvEntry, SecretConfig, SecretProviderConfig, parse};
use providers::{ParameterStoreProvider, Provider, SecretsManagerProvider};
use regex::Regex;

/// Returns a map of key value pairs after resolving all secrets
/// and applying placeholders and overrides.
pub async fn process_entries<'a>(
    mut entries: EnvEntries<'a>,
    overrides: &'a IndexMap<String, String>,
    placeholders: &'a HashMap<String, String>,
) -> Result<IndexMap<&'a str, Cow<'a, str>>, Error> {
    let mut sm_entries = vec![];
    let mut ps_entries = vec![];

    for (i, entry) in entries.iter().enumerate() {
        match entry.secret {
            Some(SecretConfig {
                provider_config: SecretProviderConfig::AwsSm(id),
                ..
            }) => {
                sm_entries.push((i, replace_placeholders(id, placeholders)?));
            }
            Some(SecretConfig {
                provider_config: SecretProviderConfig::AwsPs(id),
                ..
            }) => {
                ps_entries.push((i, replace_placeholders(id, placeholders)?));
            }
            None => {}
        }
    }

    if !sm_entries.is_empty() {
        let provider = SecretsManagerProvider::new().await;
        let secrets = provider
            .try_provide_secrets(sm_entries.iter().map(|(_, id)| id.clone()).collect())
            .await?;

        for ((i, id), secret) in sm_entries.into_iter().zip(secrets) {
            match (secret, &entries[i].secret) {
                (_, None) => unreachable!(),
                (Some(secret), _) => entries[i].value = Some(Cow::Owned(secret)),
                (None, Some(SecretConfig { required: true, .. })) => {
                    return Err(Error::ParameterNotFound(id));
                }
                _ => {}
            };
        }
    }

    if !ps_entries.is_empty() {
        let provider = ParameterStoreProvider::new().await;
        let secrets = provider
            .try_provide_secrets(ps_entries.iter().map(|(_, id)| id.clone()).collect())
            .await?;

        for ((i, id), secret) in ps_entries.into_iter().zip(secrets) {
            match (secret, &entries[i].secret) {
                (_, None) => unreachable!(),
                (Some(secret), _) => entries[i].value = Some(Cow::Owned(secret)),
                (None, Some(SecretConfig { required: true, .. })) => {
                    return Err(Error::ParameterNotFound(id));
                }
                _ => {}
            };
        }
    }

    let mut result: IndexMap<&'a str, Cow<'a, str>> = entries
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

static RE_PLACEHOLDER: OnceLock<Regex> = OnceLock::new();
static MARKER: &str = "\u{FFFF}ESCAPED\u{FFFF}";

fn replace_placeholders(id: &str, placeholders: &HashMap<String, String>) -> Result<String, Error> {
    let re = RE_PLACEHOLDER.get_or_init(|| Regex::new(r"\$(\w+)").unwrap());
    let output = Cow::Owned(id.replace("$$", MARKER));

    let mut missing: Option<Error> = None;

    let mut output = re.replace_all(&output, |caps: &regex::Captures| {
        let name = caps
            .get(1)
            .expect("a match should contain a capture")
            .as_str();

        match placeholders.get(name) {
            Some(value) => value,
            None => {
                missing = Some(Error::PlaceholderMissing(name.to_owned()));
                ""
            }
        }
    });

    if let Some(err) = missing {
        return Err(err);
    }

    Ok(output.to_mut().replace(MARKER, "$"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replaces_placeholders() {
        let input = "$foo/bar/$baz";
        let mut placeholders = HashMap::new();

        placeholders.insert("foo".to_string(), "123".to_string());
        placeholders.insert("baz".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert_eq!(result, Ok("123/bar/456".to_string()))
    }

    #[test]
    fn test_handles_escapes() {
        let input = "$$foo/bar/$baz";
        let mut placeholders = HashMap::new();

        placeholders.insert("foo".to_string(), "123".to_string());
        placeholders.insert("baz".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert_eq!(result, Ok("$foo/bar/456".to_string()))
    }

    #[test]
    fn test_returns_error_for_missing_placeholder() {
        let input = "$foo/bar/$baz";
        let mut placeholders = HashMap::new();

        placeholders.insert("baz".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert!(result.is_err())
    }

    #[test]
    fn test_supports_underscores_in_placeholders() {
        let input = "bar/$baz_1";
        let mut placeholders = HashMap::new();

        placeholders.insert("baz_1".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert_eq!(result, Ok("bar/456".to_string()))
    }
}
