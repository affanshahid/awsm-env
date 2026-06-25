use std::{fs::File, io};

use crate::{output::Output, parser::EnvParser, variable::Variables};

use anyhow::Result;

/// Formats environment variables into shell variable export commands using [`ShellOutput::format`]
pub struct ShellOutput;

impl Output for ShellOutput {
    /// Formats environment variables into shell variable export commands
    fn format(&self, variables: Variables) -> Result<String> {
        let mut output = String::new();

        for var in variables {
            output.push_str(&format!(
                "export {}={}\n",
                var.key,
                serde_json::to_string(&var.value.or(var.default))?
            ));
        }

        Ok(output)
    }

    /// Loads existing environment variables from a file in shell variable export command format
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
    fn test_shell_output() {
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

        let output = ShellOutput;
        let result = output.format(input).unwrap();
        assert_eq!(
            result,
            "export KEY1=\"value1\"\nexport KEY2=\"val\\\"ue2\"\n"
        )
    }

    #[test]
    fn test_shell_output_default_fallback() {
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

        let result = ShellOutput.format(input).unwrap();
        assert_eq!(result, "export ONLY_DEFAULT=\"def\"\nexport BOTH=\"val\"\n");
    }

    #[test]
    fn test_shell_load_existing() {
        let path = write_temp(
            "shell_load.sh",
            "export KEY1=\"value1\"\nexport KEY2=\"val\\\"ue2\"\n",
        );

        let result = ShellOutput
            .load_existing(File::open(&path).unwrap())
            .unwrap();

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
