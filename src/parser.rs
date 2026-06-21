use crate::variable::{ProviderConfig, Variable, Variables};
use anyhow::Result;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "env.pest"]
pub struct EnvParser;

impl EnvParser {
    /// Parses a string representing the contents of
    /// an .env file returning [`EnvEntries`]
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let input = r#"
    /// # @aws-sm foobar/123
    /// KEY1=value1
    /// # @aws-sm barbaz/456
    /// KEY2=value2
    /// "#;
    /// let result = EnvParser::parse_variables(input);
    ///
    /// assert_eq!(
    ///     *result.unwrap(),
    ///     vec![
    ///         Variable {
    ///             key: "KEY1".to_owned(),
    ///             required: true,
    ///             default: Some("value1".to_owned()),
    ///             provider_config: Some(ProviderConfig::AwsSecretsManager("foobar/123".to_owned())),
    ///             ..Default::default()
    ///         },
    ///         Variable {
    ///             key: "KEY2".to_owned(),
    ///             required: true,
    ///             default: Some("value2".to_owned()),
    ///             provider_config: Some(ProviderConfig::AwsSecretsManager("barbaz/456".to_owned())),
    ///             ..Default::default()
    ///         }
    ///     ]
    /// )
    /// ```
    pub fn parse_variables(input: &str) -> Result<Variables> {
        let file = EnvParser::parse(Rule::file, input)?
            .next()
            .expect("should have one file");

        let mut variables = Variables::new();

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
                            raw_value.replace("\\\"", "\"")
                        }
                        Rule::pair_value_squote if raw_value.contains("\\'") => {
                            raw_value.replace("\\'", "'")
                        }
                        Rule::pair_value_tick if raw_value.contains("\\`") => {
                            raw_value.replace("\\`", "`")
                        }
                        Rule::pair_value_raw => raw_value.trim().to_owned(),
                        Rule::pair_value_squote
                        | Rule::pair_value_dquote
                        | Rule::pair_value_tick => raw_value.to_owned(),
                        _ => unreachable!(),
                    };

                    let default = if pair_value.is_empty() {
                        None
                    } else {
                        Some(pair_value)
                    };

                    let (required, config) = match directive {
                        Some(directive) => {
                            let mut pairs = directive.into_inner();
                            let inner_directive =
                                pairs.next().expect("should have inner directive");

                            let config = match inner_directive.as_rule() {
                                Rule::aws_sm_directive => ProviderConfig::AwsSecretsManager(
                                    inner_directive
                                        .into_inner()
                                        .next()
                                        .expect("should have value")
                                        .to_string(),
                                ),
                                Rule::aws_ps_directive => ProviderConfig::AwsSecretsManager(
                                    inner_directive
                                        .into_inner()
                                        .next()
                                        .expect("should have value")
                                        .to_string(),
                                ),
                                _ => unreachable!(),
                            };

                            let optional_indicator = pairs.next();

                            (optional_indicator.is_none(), Some(config))
                        }
                        None => (true, None),
                    };

                    let variable = Variable {
                        key: pair_ident.to_owned(),
                        required,
                        default,
                        provider_config: config,
                        ..Default::default()
                    };

                    variables.push(variable);
                }
                Rule::EOI => (),
                _ => unreachable!(),
            }
        }

        Ok(variables)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parses_key_value() {
        let input = r#"
            KEY1=value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: true,
                default: Some("value1".to_owned()),
                ..Default::default()
            }]
        )
    }

    #[test]
    fn test_parses_key_value_with_colon() {
        let input = r#"
            KEY1:value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: true,
                default: Some("value1".to_owned()),
                ..Default::default()
            }]
        )
    }

    #[test]
    fn test_parses_multiple_key_value() {
        let input = r#"
            KEY1=value1
            KEY2=value2
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("value1".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY2".to_owned(),
                    required: true,
                    default: Some("value2".to_owned()),
                    ..Default::default()
                }
            ]
        )
    }

    #[test]
    fn test_handles_export_before_key() {
        let input = r#"
            export KEY1=value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: true,
                default: Some("value1".to_owned()),
                ..Default::default()
            }]
        )
    }

    #[test]
    fn test_parses_aws_sm_directive() {
        let input = r#"
            # @aws-sm foobar/123
            KEY1=value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: true,
                default: Some("value1".to_owned()),
                provider_config: Some(ProviderConfig::AwsSecretsManager("foobar/123".to_owned())),
                ..Default::default()
            }]
        )
    }

    #[test]
    fn test_parses_aws_ps_directive() {
        let input = r#"
            # @aws-ps foobar/123
            KEY1=value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: true,
                default: Some("value1".to_owned()),
                provider_config: Some(ProviderConfig::AwsParameterStore("foobar/123".to_owned())),
                ..Default::default()
            }]
        )
    }

    #[test]
    fn test_parses_optional_directive() {
        let input = r#"
            # @aws-ps foobar/123 @optional
            KEY1=value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: false,
                default: Some("value1".to_owned()),
                provider_config: Some(ProviderConfig::AwsParameterStore("foobar/123".to_owned())),
                ..Default::default()
            }]
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
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("value1".to_owned()),
                    provider_config: Some(ProviderConfig::AwsSecretsManager(
                        "foobar/123".to_owned()
                    )),
                    ..Default::default()
                },
                Variable {
                    key: "KEY2".to_owned(),
                    required: true,
                    default: Some("value2".to_owned()),
                    provider_config: Some(ProviderConfig::AwsSecretsManager(
                        "barbaz/456".to_owned()
                    )),
                    ..Default::default()
                }
            ]
        )
    }

    #[test]
    fn test_handles_spacing() {
        let input = r#"
            #    @aws-sm     foobar/123
            KEY1 =   value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: true,
                default: Some("value1".to_owned()),
                provider_config: Some(ProviderConfig::AwsSecretsManager("foobar/123".to_owned())),
                ..Default::default()
            },]
        )
    }

    #[test]
    fn test_ignores_comments_after_directive() {
        let input = r#"
            # @aws-sm foobar/123
            # test
            KEY1=value1
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![Variable {
                key: "KEY1".to_owned(),
                required: true,
                default: Some("value1".to_owned()),
                provider_config: Some(ProviderConfig::AwsSecretsManager("foobar/123".to_owned())),
                ..Default::default()
            }]
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
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("value1".to_owned()),
                    provider_config: Some(ProviderConfig::AwsSecretsManager(
                        "foobar/123".to_owned()
                    )),
                    ..Default::default()
                },
                Variable {
                    key: "KEY2".to_owned(),
                    required: true,
                    default: Some("value2".to_owned()),
                    provider_config: Some(ProviderConfig::AwsSecretsManager(
                        "barbaz/456".to_owned()
                    )),
                    ..Default::default()
                }
            ]
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
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("value1".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY2".to_owned(),
                    required: true,
                    default: Some("value2".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY3".to_owned(),
                    required: true,
                    default: Some("value3".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY4".to_owned(),
                    required: true,
                    default: Some("value4".to_owned()),
                    ..Default::default()
                }
            ]
        )
    }

    #[test]
    fn test_handles_escaped_delimiters() {
        let input = r#"
            KEY1="val\"ue1"
            KEY2='val\'ue2'
            KEY3=`val\`ue3`
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("val\"ue1".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY2".to_owned(),
                    required: true,
                    default: Some("val'ue2".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY3".to_owned(),
                    required: true,
                    default: Some("val`ue3".to_owned()),
                    ..Default::default()
                }
            ]
        )
    }

    #[test]
    fn test_does_not_allow_unescaped_dquote() {
        let input = r#"
            KEY1="val"ue1"
        "#;
        let result = EnvParser::parse_variables(input);

        assert!(result.is_err())
    }

    #[test]
    fn test_does_not_allow_unescaped_squote() {
        let input = r#"
            KEY1='val'ue1'
        "#;
        let result = EnvParser::parse_variables(input);

        assert!(result.is_err())
    }

    #[test]
    fn test_does_not_allow_unescaped_tick() {
        let input = r#"
            KEY1=`val`ue1`
        "#;
        let result = EnvParser::parse_variables(input);

        assert!(result.is_err())
    }

    #[test]
    fn test_handles_empty_values() {
        let input = r#"
            KEY1=value1
            KEY2=
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("value1".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY2".to_owned(),
                    required: true,
                    ..Default::default()
                }
            ]
        )
    }

    #[test]
    fn test_keeps_duplicate_keys() {
        let input = r#"
            KEY1=value1
            KEY1=overridden
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("value1".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("overridden".to_owned()),
                    ..Default::default()
                }
            ]
        )
    }

    #[test]
    fn test_preserves_spaces_in_quotes() {
        let input = r#"
            KEY1="  val  ue  1  "
            KEY2='  val  ue  2  '
            KEY3=`  val  ue  3  `
        "#;
        let result = EnvParser::parse_variables(input);

        assert_eq!(
            *result.unwrap(),
            vec![
                Variable {
                    key: "KEY1".to_owned(),
                    required: true,
                    default: Some("  val  ue  1  ".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY2".to_owned(),
                    required: true,
                    default: Some("  val  ue  2  ".to_owned()),
                    ..Default::default()
                },
                Variable {
                    key: "KEY3".to_owned(),
                    required: true,
                    default: Some("  val  ue  3  ".to_owned()),
                    ..Default::default()
                }
            ]
        )
    }
}
