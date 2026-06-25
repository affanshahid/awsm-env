use std::{fs::File, io};

use crate::{output::Output, parser::EnvParser, variable::Variables};

use anyhow::Result;

/// Formats environment variables into `.env` format using [`EnvOutput::format`]
pub struct EnvOutput;

impl Output for EnvOutput {
    /// Formats environment variables into `.env` format
    fn format(&self, variables: Variables) -> Result<String> {
        let mut output = String::new();

        for var in variables {
            output.push_str(&format!(
                "{}={}\n",
                var.key,
                serde_json::to_string(&var.value.or(var.default))?
            ));
        }

        Ok(output)
    }

    /// Loads existing environment variables from a file in `.env` format
    fn load_existing(&self, file: File) -> Result<Variables> {
        let input = io::read_to_string(file)?;
        let mut variables = EnvParser::parse_variables(&input)?;
        variables.iter_mut().for_each(|v| v.promote_default());
        Ok(variables)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::variable::Variable;

    use super::*;

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("awsm_env_test_{}", name));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn test_env_output() {
        let input: Variables = vec![
            Variable {
                key: "KEY1".to_string(),
                required: true,
                value: Some("value1".to_string()),
                ..Default::default()
            },
            Variable {
                key: "KEY2".to_string(),
                required: true,
                value: Some("val\"ue2".to_string()),
                ..Default::default()
            },
        ]
        .into();

        let output = EnvOutput;
        let result = output.format(input).unwrap();
        assert_eq!(result, "KEY1=\"value1\"\nKEY2=\"val\\\"ue2\"\n")
    }

    #[test]
    fn test_env_output_default_fallback() {
        // The default is used when `value` is missing, and `value` wins when
        // both the default and value are present.
        let input: Variables = vec![
            Variable {
                key: "ONLY_DEFAULT".to_string(),
                default: Some("def".to_string()),
                ..Default::default()
            },
            Variable {
                key: "BOTH".to_string(),
                default: Some("def".to_string()),
                value: Some("val".to_string()),
                ..Default::default()
            },
        ]
        .into();

        let result = EnvOutput.format(input).unwrap();
        assert_eq!(result, "ONLY_DEFAULT=\"def\"\nBOTH=\"val\"\n");
    }

    #[test]
    fn test_env_load_existing() {
        let path = write_temp("env_load.env", "KEY1=\"value1\"\nKEY2=\"val\\\"ue2\"\n");

        let result = EnvOutput.load_existing(File::open(&path).unwrap()).unwrap();

        // `load_existing` promotes parsed defaults into `value`, and plain
        // entries are parsed as `required`.
        let expected: Variables = vec![
            Variable {
                key: "KEY1".to_string(),
                required: true,
                value: Some("value1".to_string()),
                ..Default::default()
            },
            Variable {
                key: "KEY2".to_string(),
                required: true,
                value: Some("val\"ue2".to_string()),
                ..Default::default()
            },
        ]
        .into();
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path);
    }
}
