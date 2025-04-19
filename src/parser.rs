use std::borrow::Cow;

use crate::error::Error;
use indexmap::IndexMap;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "env.pest"]
struct EnvParser;

#[derive(Debug, PartialEq, Eq)]
pub enum SecretProviderConfig<'a> {
    AwsSm(&'a str),
    AwsPs(&'a str),
}

#[derive(Debug, PartialEq, Eq)]
pub struct SecretConfig<'a> {
    pub required: bool,
    pub provider_config: SecretProviderConfig<'a>,
}

/// Represents a single env entry.
#[derive(Debug, PartialEq, Eq)]
pub struct EnvEntry<'a> {
    pub key: &'a str,
    pub value: Option<Cow<'a, str>>,
    pub secret: Option<SecretConfig<'a>>,
}

/// List of [`EnvEntry`]s.
pub type EnvEntries<'a> = Vec<EnvEntry<'a>>;

/// Parses a string representing the contents of
/// an .env file returning [`EnvEntries`]
///
/// # Examples
///
/// ```
/// # use awsm_env::*;
/// # use std::borrow::Cow;
///
/// let input = r#"
/// ## @aws-sm foobar/123
/// KEY1=value1
/// ## @aws-sm barbaz/456
/// KEY2=value2
/// "#;
/// let result = parse(&input);
///
/// assert_eq!(
///     result,
///     Ok(vec![
///         EnvEntry {
///             key: "KEY1",
///             value: Some(Cow::Borrowed("value1")),
///             secret: Some(SecretConfig {
///                 required: true,
///                 provider_config: SecretProviderConfig::AwsSm("foobar/123")
///             })
///         },
///         EnvEntry {
///             key: "KEY2",
///             value: Some(Cow::Borrowed("value2")),
///             secret: Some(SecretConfig {
///                 required: true,
///                 provider_config: SecretProviderConfig::AwsSm("barbaz/456")
///             })
///         }
///     ])
/// )
/// ```
pub fn parse(input: &str) -> Result<EnvEntries, Error> {
    let file = EnvParser::parse(Rule::file, input)?
        .next()
        .expect("should have one file");

    let mut entries = IndexMap::new();

    for line in file.into_inner() {
        match line.as_rule() {
            Rule::declaration => {
                let mut pairs = line.into_inner();
                let (directive, pair) = match (pairs.next(), pairs.next()) {
                    (Some(directive), Some(pair)) => (Some(directive), pair),
                    (Some(pair), None) => (None, pair),
                    _ => unreachable!(),
                };

                let mut pairs = pair.into_inner();

                let pair_ident = pairs.next().expect("should have pair_ident").as_str();
                let pair_value = pairs
                    .next()
                    .expect("should have pair_value")
                    .into_inner()
                    .next()
                    .expect("should have inner value");

                let raw_value = pair_value.as_str();

                let pair_value = match pair_value.as_rule() {
                    Rule::pair_value_dquote if raw_value.contains("\\\"") => {
                        Cow::Owned(raw_value.replace("\\\"", "\""))
                    }
                    Rule::pair_value_squote if raw_value.contains("\\'") => {
                        Cow::Owned(raw_value.replace("\\'", "'"))
                    }
                    Rule::pair_value_tick if raw_value.contains("\\`") => {
                        Cow::Owned(raw_value.replace("\\`", "`"))
                    }
                    Rule::pair_value_raw => Cow::Borrowed(raw_value.trim()),
                    Rule::pair_value_squote | Rule::pair_value_dquote | Rule::pair_value_tick => {
                        Cow::Borrowed(raw_value)
                    }
                    _ => unreachable!(),
                };

                let default = if pair_value.is_empty() {
                    None
                } else {
                    Some(pair_value)
                };

                let secret = directive.map(|directive| {
                    let mut pairs = directive.into_inner();
                    let inner_directive = pairs.next().expect("should have inner directive");

                    let config = match inner_directive.as_rule() {
                        Rule::aws_sm_directive => SecretProviderConfig::AwsSm(
                            inner_directive
                                .into_inner()
                                .next()
                                .expect("should have value")
                                .as_str(),
                        ),
                        Rule::aws_ps_directive => SecretProviderConfig::AwsPs(
                            inner_directive
                                .into_inner()
                                .next()
                                .expect("should have value")
                                .as_str(),
                        ),
                        _ => unreachable!(),
                    };

                    let optional_indicator = pairs.next();

                    SecretConfig {
                        required: optional_indicator.is_none(),
                        provider_config: config,
                    }
                });

                let entry = EnvEntry {
                    key: pair_ident,
                    value: default,
                    secret,
                };

                if entries.contains_key(pair_ident) {
                    eprintln!("Warning: Duplicate declaration for {pair_ident}");
                }

                entries.insert(pair_ident, entry);
            }
            Rule::EOI => (),
            _ => unreachable!(),
        }
    }

    Ok(entries.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_key_value() {
        let input = r#"
            KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: None
            }])
        )
    }

    #[test]
    fn test_parses_key_value_with_colon() {
        let input = r#"
            KEY1:value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: None
            }])
        )
    }

    #[test]
    fn test_parses_multiple_key_value() {
        let input = r#"
            KEY1=value1
            KEY2=value2
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    value: Some(Cow::Borrowed("value1")),
                    secret: None
                },
                EnvEntry {
                    key: "KEY2",
                    value: Some(Cow::Borrowed("value2")),
                    secret: None
                }
            ])
        )
    }

    #[test]
    fn test_handles_export_before_key() {
        let input = r#"
            export KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: None
            }])
        )
    }

    #[test]
    fn test_parses_aws_sm_directive() {
        let input = r#"
            # @aws-sm foobar/123
            KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: Some(SecretConfig {
                    required: true,
                    provider_config: SecretProviderConfig::AwsSm("foobar/123")
                })
            }])
        )
    }

    #[test]
    fn test_parses_aws_ps_directive() {
        let input = r#"
            # @aws-ps foobar/123
            KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: Some(SecretConfig {
                    required: true,
                    provider_config: SecretProviderConfig::AwsPs("foobar/123")
                })
            }])
        )
    }

    #[test]
    fn test_parses_optional_directive() {
        let input = r#"
            # @aws-ps foobar/123 @optional
            KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: Some(SecretConfig {
                    required: false,
                    provider_config: SecretProviderConfig::AwsPs("foobar/123")
                })
            }])
        )
    }

    #[test]
    fn test_parses_multiple_directives() {
        let input = r#"
            # @aws-sm foobar/123
            KEY1=value1

            # @aws-sm barbaz/456
            KEY2=value2
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    value: Some(Cow::Borrowed("value1")),
                    secret: Some(SecretConfig {
                        required: true,
                        provider_config: SecretProviderConfig::AwsSm("foobar/123")
                    })
                },
                EnvEntry {
                    key: "KEY2",
                    value: Some(Cow::Borrowed("value2")),
                    secret: Some(SecretConfig {
                        required: true,
                        provider_config: SecretProviderConfig::AwsSm("barbaz/456")
                    })
                }
            ])
        )
    }

    #[test]
    fn test_handles_spacing() {
        let input = r#"
            #    @aws-sm     foobar/123
            KEY1 =   value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: Some(SecretConfig {
                    required: true,
                    provider_config: SecretProviderConfig::AwsSm("foobar/123")
                })
            },])
        )
    }

    #[test]
    fn test_ignores_comments_after_directive() {
        let input = r#"
            # @aws-sm foobar/123
            # test
            KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("value1")),
                secret: Some(SecretConfig {
                    required: true,
                    provider_config: SecretProviderConfig::AwsSm("foobar/123")
                })
            }])
        )
    }

    #[test]
    fn test_ignores_comments_between_entries() {
        let input = r#"
            #test
            # @aws-sm foobar/123
            KEY1=value1
            # test
            # @aws-sm barbaz/456
            KEY2=value2
            # test
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    value: Some(Cow::Borrowed("value1")),
                    secret: Some(SecretConfig {
                        required: true,
                        provider_config: SecretProviderConfig::AwsSm("foobar/123")
                    })
                },
                EnvEntry {
                    key: "KEY2",
                    value: Some(Cow::Borrowed("value2")),
                    secret: Some(SecretConfig {
                        required: true,
                        provider_config: SecretProviderConfig::AwsSm("barbaz/456")
                    })
                }
            ])
        )
    }

    #[test]
    fn test_handles_comments_at_the_end_of_values() {
        let input = r#"
            KEY1=value1 # test 123
            KEY2="value2" # test 123
            KEY3='value3' # test 123
            KEY4=`value4` # test 123
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    value: Some(Cow::Borrowed("value1")),
                    secret: None
                },
                EnvEntry {
                    key: "KEY2",
                    value: Some(Cow::Borrowed("value2")),
                    secret: None
                },
                EnvEntry {
                    key: "KEY3",
                    value: Some(Cow::Borrowed("value3")),
                    secret: None
                },
                EnvEntry {
                    key: "KEY4",
                    value: Some(Cow::Borrowed("value4")),
                    secret: None
                }
            ])
        )
    }

    #[test]
    fn test_handles_escaped_delimiters() {
        let input = r#"
            KEY1="val\"ue1"
            KEY2='val\'ue2'
            KEY3=`val\`ue3`
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    value: Some(Cow::Owned("val\"ue1".to_string())),
                    secret: None
                },
                EnvEntry {
                    key: "KEY2",
                    value: Some(Cow::Owned("val'ue2".to_string())),
                    secret: None
                },
                EnvEntry {
                    key: "KEY3",
                    value: Some(Cow::Owned("val`ue3".to_string())),
                    secret: None
                }
            ])
        )
    }

    #[test]
    fn test_does_not_allow_unescaped_dquote() {
        let input = r#"
            KEY1="val"ue1"
        "#;
        let result = parse(&input);

        assert!(result.is_err())
    }

    #[test]
    fn test_does_not_allow_unescaped_squote() {
        let input = r#"
            KEY1='val'ue1'
        "#;
        let result = parse(&input);

        assert!(result.is_err())
    }

    #[test]
    fn test_does_not_allow_unescaped_tick() {
        let input = r#"
            KEY1=`val`ue1`
        "#;
        let result = parse(&input);

        assert!(result.is_err())
    }

    #[test]
    fn test_handles_empty_values() {
        let input = r#"
            KEY1=value1
            KEY2=
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    value: Some(Cow::Borrowed("value1")),
                    secret: None
                },
                EnvEntry {
                    key: "KEY2",
                    value: None,
                    secret: None
                }
            ])
        )
    }

    #[test]
    fn test_overrides_duplicate_keys() {
        let input = r#"
            KEY1=value1
            KEY1=overridden
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                value: Some(Cow::Borrowed("overridden")),
                secret: None
            },])
        )
    }

    #[test]
    fn test_preserves_spaces_in_quotes() {
        let input = r#"
            KEY1="  val  ue  1  "
            KEY2='  val  ue  2  '
            KEY3=`  val  ue  3  `
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    value: Some(Cow::Borrowed("  val  ue  1  ")),
                    secret: None
                },
                EnvEntry {
                    key: "KEY2",
                    value: Some(Cow::Borrowed("  val  ue  2  ")),
                    secret: None
                },
                EnvEntry {
                    key: "KEY3",
                    value: Some(Cow::Borrowed("  val  ue  3  ")),
                    secret: None
                }
            ])
        )
    }
}
