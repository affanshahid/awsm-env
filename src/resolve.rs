use std::sync::OnceLock;

use anyhow::{Error, Result, anyhow};
use indexmap::IndexMap;
use itertools::Itertools;
use regex::Regex;

use crate::{
    cli::MergeMode,
    provider::{AwsParameterStoreProvider, AwsSecretsManagerProvider, Provider},
    variable::{ProviderConfig, Variables},
};

static RE_PLACEHOLDER: OnceLock<Regex> = OnceLock::new();
static MARKER: &str = "\u{FFFF}ESCAPED\u{FFFF}";

#[derive(Eq, PartialEq, Hash)]
enum ProviderKind {
    AwsSecretsManager,
    AwsParameterStore,
}

impl From<&ProviderConfig> for ProviderKind {
    fn from(value: &ProviderConfig) -> Self {
        match value {
            ProviderConfig::AwsSecretsManager(_) => ProviderKind::AwsSecretsManager,
            ProviderConfig::AwsParameterStore(_) => ProviderKind::AwsParameterStore,
        }
    }
}

pub async fn resolve(
    variables: &mut Variables,
    placeholders: IndexMap<String, String>,
) -> Result<()> {
    let groups = variables
        .iter_mut()
        .into_group_map_by(|v| v.provider_config.as_ref().map(ProviderKind::from));

    let aws_sm = AwsSecretsManagerProvider::new().await;
    let aws_ps = AwsParameterStoreProvider::new().await;

    for (kind, mut group) in groups {
        let provider_kind = match kind {
            Some(k) => k,
            None => continue,
        };

        let ids = group
            .iter()
            .map(|v| {
                v.provider_config
                    .as_ref()
                    .expect("Expected nones to be filtered out")
                    .id()
            })
            .map(|id| replace_placeholders(id, &placeholders))
            .collect::<Result<Vec<_>>>()?;

        let resolved = match provider_kind {
            ProviderKind::AwsSecretsManager => aws_sm.provide_secrets(ids).await?,
            ProviderKind::AwsParameterStore => aws_ps.provide_secrets(ids).await?,
        };

        for secret in resolved {
            let var = group
                .iter_mut()
                .find(|v| v.key == secret.id)
                .expect("Expected matching variable");

            var.value = Some(secret.secret);
        }
    }

    Ok(())
}

pub fn merge(mut variables: Variables, mut others: Variables, mode: MergeMode) -> Variables {
    match mode {
        MergeMode::Overwrite => variables,
        MergeMode::Fallback => {
            others.iter_mut().for_each(|o| o.demote_value());
            variables.merge(others);
            variables
        }
        MergeMode::Override => {
            variables.merge(others);
            variables
        }
    }
}

fn replace_placeholders(id: &str, placeholders: &IndexMap<String, String>) -> Result<String> {
    let re = RE_PLACEHOLDER.get_or_init(|| Regex::new(r"\$(\w+)").unwrap());
    let output = id.replace("$$", MARKER);

    let mut missing: Option<Error> = None;

    let mut output = re.replace_all(&output, |caps: &regex::Captures| {
        let name = caps
            .get(1)
            .expect("a match should contain a capture")
            .as_str();

        match placeholders.get(name) {
            Some(value) => value,
            None => {
                missing = Some(anyhow!("Missing placeholder: {}", name));
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
    use crate::variable::Variable;

    use super::*;

    #[test]
    fn test_replaces_placeholders() {
        let input = "$foo/bar/$baz";
        let mut placeholders = IndexMap::new();

        placeholders.insert("foo".to_string(), "123".to_string());
        placeholders.insert("baz".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert_eq!(result.unwrap(), "123/bar/456".to_string())
    }

    #[test]
    fn test_handles_escapes() {
        let input = "$$foo/bar/$baz";
        let mut placeholders = IndexMap::new();

        placeholders.insert("foo".to_string(), "123".to_string());
        placeholders.insert("baz".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert_eq!(result.unwrap(), "$foo/bar/456".to_string())
    }

    #[test]
    fn test_returns_error_for_missing_placeholder() {
        let input = "$foo/bar/$baz";
        let mut placeholders = IndexMap::new();

        placeholders.insert("baz".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert!(result.is_err())
    }

    #[test]
    fn test_supports_underscores_in_placeholders() {
        let input = "bar/$baz_1";
        let mut placeholders = IndexMap::new();

        placeholders.insert("baz_1".to_string(), "456".to_string());

        let result = replace_placeholders(input, &placeholders);

        assert_eq!(result.unwrap(), "bar/456".to_string())
    }

    fn var(key: &str, value: &str) -> Variable {
        Variable {
            key: key.to_string(),
            value: Some(value.to_string()),
            ..Default::default()
        }
    }

    fn vars(items: Vec<Variable>) -> Variables {
        let mut v = Variables::new();
        for item in items {
            v.insert(item);
        }
        v
    }

    /// Primary set: the variables being merged into.
    fn base() -> Variables {
        vars(vec![var("SHARED", "base"), var("ONLY_BASE", "b")])
    }

    /// Secondary set: the variables merged in.
    fn other() -> Variables {
        vars(vec![var("SHARED", "other"), var("ONLY_OTHER", "o")])
    }

    fn keys(variables: &Variables) -> Vec<&str> {
        variables.iter().map(|v| v.key.as_str()).collect()
    }

    #[test]
    fn test_merge_overwrite_ignores_other() {
        let result = merge(base(), other(), MergeMode::Overwrite);

        assert_eq!(result.len(), 2);
        assert_eq!(
            result.find_by_key("SHARED").unwrap().value.as_deref(),
            Some("base")
        );
        assert_eq!(
            result.find_by_key("ONLY_BASE").unwrap().value.as_deref(),
            Some("b")
        );
        assert!(result.find_by_key("ONLY_OTHER").is_none());
    }

    #[test]
    fn test_merge_override_other_wins() {
        let result = merge(base(), other(), MergeMode::Override);

        assert_eq!(
            result.find_by_key("SHARED").unwrap().value.as_deref(),
            Some("other")
        );
        assert_eq!(
            result.find_by_key("ONLY_BASE").unwrap().value.as_deref(),
            Some("b")
        );
        assert_eq!(
            result.find_by_key("ONLY_OTHER").unwrap().value.as_deref(),
            Some("o")
        );

        assert_eq!(keys(&result), vec!["SHARED", "ONLY_BASE", "ONLY_OTHER"]);
    }

    #[test]
    fn test_merge_fallback_base_wins_other_demotes_to_default() {
        let result = merge(base(), other(), MergeMode::Fallback);

        // Shared key: base keeps its value, other's value becomes the fallback default.
        let shared = result.find_by_key("SHARED").unwrap();
        assert_eq!(shared.value.as_deref(), Some("base"));
        assert_eq!(shared.default.as_deref(), Some("other"));

        assert_eq!(
            result.find_by_key("ONLY_BASE").unwrap().value.as_deref(),
            Some("b")
        );

        // Other-only key arrives as a default, not a value.
        let only_other = result.find_by_key("ONLY_OTHER").unwrap();
        assert_eq!(only_other.value, None);
        assert_eq!(only_other.default.as_deref(), Some("o"));

        assert_eq!(keys(&result), vec!["SHARED", "ONLY_BASE", "ONLY_OTHER"]);
    }

    #[test]
    fn test_merge_with_empty_other_is_identity() {
        for mode in [
            MergeMode::Overwrite,
            MergeMode::Fallback,
            MergeMode::Override,
        ] {
            let result = merge(base(), Variables::new(), mode);
            assert_eq!(result.len(), 2);
            assert_eq!(
                result.find_by_key("SHARED").unwrap().value.as_deref(),
                Some("base")
            );
            assert_eq!(
                result.find_by_key("ONLY_BASE").unwrap().value.as_deref(),
                Some("b")
            );
        }
    }
}
