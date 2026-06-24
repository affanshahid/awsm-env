use std::{fs::File, io};

use indexmap::IndexMap;

use crate::{output::Output, variable::Variables};

use anyhow::Result;

/// Formats environment variables into JSON using [`JsonOutput::format`]
pub struct JsonOutput;

impl Output for JsonOutput {
    /// Formats environment variables into JSON of the form `{"KEY": "value"}`
    fn format(&self, variables: Variables) -> Result<String> {
        let map: IndexMap<_, _> = variables.into();
        Ok(serde_json::to_string(&map)? + "\n")
    }

    /// Loads existing environment variables from a JSON file
    fn load_existing(&self, file: File) -> Result<Variables> {
        let input = io::read_to_string(file)?;
        let obj: IndexMap<_, _> = serde_json::from_str(&input)?;

        Ok(obj.into())
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
    fn test_json_output() {
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

        let output = JsonOutput;
        let result = output.format(input).unwrap();

        let expected = serde_json::json!({
            "KEY1": "value1",
            "KEY2": "val\"ue2",
        });
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            expected
        )
    }

    #[test]
    fn test_json_load_existing() {
        let path = write_temp(
            "json_load.json",
            "{\"KEY1\":\"value1\",\"KEY2\":\"val\\\"ue2\"}\n",
        );

        let result = JsonOutput
            .load_existing(File::open(&path).unwrap())
            .unwrap();

        // JSON values are loaded straight into `value` (no directives/required).
        let expected: Variables = vec![
            Variable {
                key: "KEY1".to_string(),
                value: Some("value1".to_string()),
                ..Default::default()
            },
            Variable {
                key: "KEY2".to_string(),
                value: Some("val\"ue2".to_string()),
                ..Default::default()
            },
        ]
        .into();
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path);
    }
}
