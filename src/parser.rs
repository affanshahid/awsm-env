use std::borrow::Cow;

use crate::error::Error;
use indexmap::IndexMap;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "env.pest"]
struct EnvParser;

/// Represents a single env entry.
#[derive(Debug, PartialEq, Eq)]
pub struct EnvEntry<'a> {
    pub key: &'a str,
    pub default: Option<Cow<'a, str>>,
    pub secret_id: Option<&'a str>,
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
/// ## @aws foobar/123
/// KEY1=value1
/// ## @aws barbaz/456
/// KEY2=value2
/// "#;
/// let result = parse(&input);
///
/// assert_eq!(
///     result,
///     Ok(vec![
///         EnvEntry {
///             key: "KEY1",
///             default: Some(Cow::Borrowed("value1")),
///             secret_id: Some("foobar/123")
///         },
///         EnvEntry {
///             key: "KEY2",
///             default: Some(Cow::Borrowed("value2")),
///               secret_id: Some("barbaz/456")
///         }
///     ])
/// )
/// ```
#[allow(clippy::result_large_err)]
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

                let secret_id = directive.map(|directive| {
                    let inner = directive
                        .into_inner()
                        .next()
                        .expect("should have inner directive");

                    match inner.as_rule() {
                        Rule::aws_directive => inner
                            .into_inner()
                            .next()
                            .expect("should have value")
                            .as_str(),
                        _ => unreachable!(),
                    }
                });

                let entry = EnvEntry {
                    key: pair_ident,
                    default,
                    secret_id,
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
                default: Some(Cow::Borrowed("value1")),
                secret_id: None
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
                default: Some(Cow::Borrowed("value1")),
                secret_id: None
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
                    default: Some(Cow::Borrowed("value1")),
                    secret_id: None
                },
                EnvEntry {
                    key: "KEY2",
                    default: Some(Cow::Borrowed("value2")),
                    secret_id: None
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
                default: Some(Cow::Borrowed("value1")),
                secret_id: None
            }])
        )
    }

    #[test]
    fn test_parses_aws_directive() {
        let input = r#"
            # @aws foobar/123
            KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                default: Some(Cow::Borrowed("value1")),
                secret_id: Some("foobar/123")
            }])
        )
    }

    #[test]
    fn test_parses_multiple_aws_directive() {
        let input = r#"
            # @aws foobar/123
            KEY1=value1

            # @aws barbaz/456
            KEY2=value2
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    default: Some(Cow::Borrowed("value1")),
                    secret_id: Some("foobar/123")
                },
                EnvEntry {
                    key: "KEY2",
                    default: Some(Cow::Borrowed("value2")),
                    secret_id: Some("barbaz/456")
                }
            ])
        )
    }

    #[test]
    fn test_handles_spacing() {
        let input = r#"
            #    @aws     foobar/123
            KEY1 =   value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                default: Some(Cow::Borrowed("value1")),
                secret_id: Some("foobar/123")
            },])
        )
    }

    #[test]
    fn test_ignores_comments_after_directive() {
        let input = r#"
            # @aws foobar/123
            # test
            KEY1=value1
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![EnvEntry {
                key: "KEY1",
                default: Some(Cow::Borrowed("value1")),
                secret_id: Some("foobar/123")
            }])
        )
    }

    #[test]
    fn test_ignores_comments_between_entries() {
        let input = r#"
            #test
            # @aws foobar/123
            KEY1=value1
            # test
            # @aws barbaz/456
            KEY2=value2
            # test
        "#;
        let result = parse(&input);

        assert_eq!(
            result,
            Ok(vec![
                EnvEntry {
                    key: "KEY1",
                    default: Some(Cow::Borrowed("value1")),
                    secret_id: Some("foobar/123")
                },
                EnvEntry {
                    key: "KEY2",
                    default: Some(Cow::Borrowed("value2")),
                    secret_id: Some("barbaz/456")
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
                    default: Some(Cow::Borrowed("value1")),
                    secret_id: None
                },
                EnvEntry {
                    key: "KEY2",
                    default: Some(Cow::Borrowed("value2")),
                    secret_id: None
                },
                EnvEntry {
                    key: "KEY3",
                    default: Some(Cow::Borrowed("value3")),
                    secret_id: None
                },
                EnvEntry {
                    key: "KEY4",
                    default: Some(Cow::Borrowed("value4")),
                    secret_id: None
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
                    default: Some(Cow::Owned("val\"ue1".to_string())),
                    secret_id: None
                },
                EnvEntry {
                    key: "KEY2",
                    default: Some(Cow::Owned("val'ue2".to_string())),
                    secret_id: None
                },
                EnvEntry {
                    key: "KEY3",
                    default: Some(Cow::Owned("val`ue3".to_string())),
                    secret_id: None
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
                    default: Some(Cow::Borrowed("value1")),
                    secret_id: None
                },
                EnvEntry {
                    key: "KEY2",
                    default: None,
                    secret_id: None
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
                default: Some(Cow::Borrowed("overridden")),
                secret_id: None
            },])
        )
    }
}
