use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    sync::OnceLock,
};

use anyhow::{Error, Result, anyhow};
use indexmap::IndexMap;
use itertools::Itertools;
use regex::Regex;

use crate::{
    cli::MergeMode,
    provider::{AwsParameterStoreProvider, AwsSecretsManagerProvider, InMemoryProvider, Provider},
    variable::{ProviderConfig, Variables},
};

static RE_PLACEHOLDER: OnceLock<Regex> = OnceLock::new();
static MARKER: &str = "\u{FFFF}ESCAPED\u{FFFF}";

#[derive(Eq, PartialEq, Hash)]
enum ProviderKind {
    AwsSecretsManager,
    AwsParameterStore,
    InMemory,
}

impl From<&ProviderConfig> for ProviderKind {
    fn from(value: &ProviderConfig) -> Self {
        match value {
            ProviderConfig::AwsSecretsManager(_) => ProviderKind::AwsSecretsManager,
            ProviderConfig::AwsParameterStore(_) => ProviderKind::AwsParameterStore,
            ProviderConfig::InMemory(_) => ProviderKind::InMemory,
        }
    }
}

#[derive(Default)]
pub struct Resolver {
    providers: HashMap<ProviderKind, Box<dyn Provider>>,
}

impl Resolver {
    pub fn new() -> Resolver {
        Resolver {
            providers: HashMap::new(),
        }
    }

    pub fn with_secrets_manager(&mut self, provider: AwsSecretsManagerProvider) -> &mut Self {
        self.add_provider(ProviderKind::AwsSecretsManager, provider)
    }

    pub fn with_parameter_store(&mut self, provider: AwsParameterStoreProvider) -> &mut Self {
        self.add_provider(ProviderKind::AwsParameterStore, provider)
    }

    pub fn with_in_memory(&mut self, provider: InMemoryProvider) -> &mut Self {
        self.add_provider(ProviderKind::InMemory, provider)
    }

    fn add_provider(&mut self, kind: ProviderKind, provider: impl Provider + 'static) -> &mut Self {
        self.providers.insert(kind, Box::new(provider));
        self
    }

    pub async fn required_by(vars: &Variables) -> Resolver {
        let mut resolver = Resolver::new();

        let kinds = vars
            .iter()
            .map(|v| v.provider_config.iter())
            .flatten()
            .map(ProviderKind::from)
            .collect::<HashSet<_>>();

        for kind in kinds {
            match kind {
                ProviderKind::AwsSecretsManager => {
                    resolver.with_secrets_manager(AwsSecretsManagerProvider::new().await);
                }
                ProviderKind::AwsParameterStore => {
                    resolver.with_parameter_store(AwsParameterStoreProvider::new().await);
                }
                ProviderKind::InMemory => {
                    resolver.with_in_memory(InMemoryProvider::new());
                }
            }
        }

        resolver
    }

    pub async fn resolve(
        &self,
        variables: &mut Variables,
        placeholders: IndexMap<String, String>,
    ) -> Result<()> {
        let groups = variables
            .iter_mut()
            .into_group_map_by(|v| v.provider_config.as_ref().map(ProviderKind::from));

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

            let resolved = self
                .providers
                .get(&provider_kind)
                .expect("Provider should be registered")
                .provide_secrets(ids)
                .await?;

            for secret in resolved {
                let vars = group.iter_mut().filter(|v| {
                    replace_placeholders(
                        v.provider_config
                            .as_ref()
                            .expect("Expected nones to be filtered out")
                            .id(),
                        &placeholders,
                    )
                    .expect("Placholder substitution succeeded earlier")
                        == secret.id
                });

                for var in vars {
                    var.value = Some(secret.secret.clone());
                }
            }
        }

        Ok(())
    }
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

    fn provider_var(key: &str, config: ProviderConfig) -> Variable {
        Variable {
            key: key.to_string(),
            provider_config: Some(config),
            ..Default::default()
        }
    }

    fn in_memory_resolver(secrets: Vec<(&str, &str)>) -> Resolver {
        let secrets: HashMap<String, String> = secrets
            .into_iter()
            .map(|(id, secret)| (id.to_string(), secret.to_string()))
            .collect();

        let mut resolver = Resolver::new();
        resolver.with_in_memory(InMemoryProvider::from_secrets(secrets));

        resolver
    }

    fn placeholders(items: Vec<(&str, &str)>) -> IndexMap<String, String> {
        items
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[tokio::test]
    async fn test_resolve_populates_values_from_provider() {
        let resolver = in_memory_resolver(vec![("secret/one", "one"), ("secret/two", "two")]);
        let mut variables = vars(vec![
            provider_var("FIRST", ProviderConfig::InMemory("secret/one".to_string())),
            provider_var("SECOND", ProviderConfig::InMemory("secret/two".to_string())),
        ]);

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(
            variables.find_by_key("FIRST").unwrap().value.as_deref(),
            Some("one")
        );
        assert_eq!(
            variables.find_by_key("SECOND").unwrap().value.as_deref(),
            Some("two")
        );
    }

    #[tokio::test]
    async fn test_resolve_leaves_variables_without_a_provider_untouched() {
        let resolver = in_memory_resolver(vec![("secret/one", "one")]);
        let mut variables = vars(vec![
            var("PLAIN", "plain"),
            provider_var("FIRST", ProviderConfig::InMemory("secret/one".to_string())),
        ]);

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(
            variables.find_by_key("PLAIN").unwrap().value.as_deref(),
            Some("plain")
        );
        assert_eq!(
            variables.find_by_key("FIRST").unwrap().value.as_deref(),
            Some("one")
        );
    }

    #[tokio::test]
    async fn test_resolve_substitutes_placeholders_in_ids() {
        let resolver = in_memory_resolver(vec![("prod/db/password", "hunter2")]);
        let mut variables = vars(vec![provider_var(
            "DB_PASSWORD",
            ProviderConfig::InMemory("$env/db/password".to_string()),
        )]);

        resolver
            .resolve(&mut variables, placeholders(vec![("env", "prod")]))
            .await
            .unwrap();

        assert_eq!(
            variables
                .find_by_key("DB_PASSWORD")
                .unwrap()
                .value
                .as_deref(),
            Some("hunter2")
        );
    }

    #[tokio::test]
    async fn test_resolve_errors_on_missing_placeholder() {
        let resolver = in_memory_resolver(vec![("prod/db/password", "hunter2")]);
        let mut variables = vars(vec![provider_var(
            "DB_PASSWORD",
            ProviderConfig::InMemory("$env/db/password".to_string()),
        )]);

        let result = resolver.resolve(&mut variables, IndexMap::new()).await;

        assert!(result.is_err());
        assert_eq!(variables.find_by_key("DB_PASSWORD").unwrap().value, None);
    }

    #[tokio::test]
    async fn test_resolve_fills_every_variable_sharing_an_id() {
        let resolver = in_memory_resolver(vec![("secret/shared", "shared")]);
        let mut variables = vars(vec![
            provider_var(
                "FIRST",
                ProviderConfig::InMemory("secret/shared".to_string()),
            ),
            provider_var(
                "SECOND",
                ProviderConfig::InMemory("secret/shared".to_string()),
            ),
        ]);

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(
            variables.find_by_key("FIRST").unwrap().value.as_deref(),
            Some("shared")
        );
        assert_eq!(
            variables.find_by_key("SECOND").unwrap().value.as_deref(),
            Some("shared")
        );
    }

    #[tokio::test]
    async fn test_resolve_leaves_unknown_ids_unset() {
        let resolver = in_memory_resolver(vec![("secret/known", "known")]);
        let mut variables = vars(vec![
            provider_var(
                "KNOWN",
                ProviderConfig::InMemory("secret/known".to_string()),
            ),
            provider_var(
                "UNKNOWN",
                ProviderConfig::InMemory("secret/missing".to_string()),
            ),
        ]);

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(
            variables.find_by_key("KNOWN").unwrap().value.as_deref(),
            Some("known")
        );
        assert_eq!(variables.find_by_key("UNKNOWN").unwrap().value, None);
    }

    #[tokio::test]
    async fn test_resolve_overwrites_existing_values() {
        let resolver = in_memory_resolver(vec![("secret/one", "resolved")]);
        let mut variables = vars(vec![Variable {
            key: "FIRST".to_string(),
            value: Some("stale".to_string()),
            provider_config: Some(ProviderConfig::InMemory("secret/one".to_string())),
            ..Default::default()
        }]);

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(
            variables.find_by_key("FIRST").unwrap().value.as_deref(),
            Some("resolved")
        );
    }

    #[tokio::test]
    async fn test_resolve_groups_by_provider_kind() {
        // Two distinct providers, each holding a different secret under the same id.
        let mut resolver = Resolver::new();
        resolver
            .with_in_memory(InMemoryProvider::from_secrets(HashMap::from([(
                "shared/id".to_string(),
                "from-memory".to_string(),
            )])))
            // The public setters are typed to their real providers, so reach for the
            // private pairing to stand a fake in for Secrets Manager.
            .add_provider(
                ProviderKind::AwsSecretsManager,
                InMemoryProvider::from_secrets(HashMap::from([(
                    "shared/id".to_string(),
                    "from-secrets-manager".to_string(),
                )])),
            );

        let mut variables = vars(vec![
            provider_var("MEM", ProviderConfig::InMemory("shared/id".to_string())),
            provider_var(
                "SM",
                ProviderConfig::AwsSecretsManager("shared/id".to_string()),
            ),
        ]);

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(
            variables.find_by_key("MEM").unwrap().value.as_deref(),
            Some("from-memory")
        );
        assert_eq!(
            variables.find_by_key("SM").unwrap().value.as_deref(),
            Some("from-secrets-manager")
        );
    }

    #[tokio::test]
    async fn test_resolve_with_no_provider_backed_variables_is_a_noop() {
        let resolver = in_memory_resolver(vec![("secret/one", "one")]);
        let mut variables = base();

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(variables, base());
    }

    #[tokio::test]
    async fn test_resolves_multiple_vars_with_same_id() {
        let mut resolver = Resolver::new();

        resolver.with_in_memory(InMemoryProvider::from_secrets(HashMap::from([(
            "shared/id".to_string(),
            "foo".to_string(),
        )])));

        let mut variables = vars(vec![
            provider_var("VAR1", ProviderConfig::InMemory("shared/id".to_string())),
            provider_var("VAR2", ProviderConfig::InMemory("shared/id".to_string())),
        ]);

        resolver
            .resolve(&mut variables, IndexMap::new())
            .await
            .unwrap();

        assert_eq!(
            variables.find_by_key("VAR1").unwrap().value.as_deref(),
            Some("foo")
        );
        assert_eq!(
            variables.find_by_key("VAR2").unwrap().value.as_deref(),
            Some("foo")
        );
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
