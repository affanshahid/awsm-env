use std::{
    fs::{self, File},
    io,
    path::PathBuf,
};

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::{output::Output, variable::Variables};

use anyhow::Result;

/// Formats environment variables into Claude Code's settings file format using [`ClaudeOutput::format`]
pub struct ClaudeOutput {
    path: PathBuf,
}

impl ClaudeOutput {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path: path.unwrap_or(PathBuf::from(".claude/settings.local.json")),
        }
    }
}

impl Output for ClaudeOutput {
    /// Formats environment variables into Claude Code's settings file format of the form `{"env": {"KEY": "value"}}`
    /// Preserves other existing settings
    fn format(&self, variables: Variables) -> Result<String> {
        let mut map = match self.path.try_exists()? {
            true => {
                let input = fs::read_to_string(&self.path)?;
                let obj: Value = serde_json::from_str(&input)?;
                serde_json::from_value(obj)?
            }
            false => Map::new(),
        };
        let env_map: IndexMap<_, _> = variables.into();
        map.insert("env".to_string(), serde_json::to_value(env_map)?);

        Ok(serde_json::to_string_pretty(&map)?)
    }

    /// Loads existing environment variables from a Claude Code settings file
    fn load_existing(&self, file: File) -> Result<Variables> {
        let input = io::read_to_string(file)?;
        let mut obj: Value = serde_json::from_str(&input)?;
        let env = obj
            .as_object_mut()
            .and_then(|o| o.remove("env"))
            .unwrap_or(Value::Object(Map::new()));
        let env: IndexMap<_, _> = serde_json::from_value(env)?;
        Ok(env.into())
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
    fn test_claude_output() {
        let path = std::env::temp_dir().join("awsm_env_test_claude_new.json");
        let _ = fs::remove_file(&path);

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

        let output = ClaudeOutput::new(Some(path.clone()));
        let result = output.format(input).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["env"]["KEY1"], "value1");
        assert_eq!(parsed["env"]["KEY2"], "val\"ue2");
    }

    #[test]
    fn test_claude_output_preserves_other_settings() {
        let path = write_temp(
            "claude_preserve.json",
            r#"{"other":"keep","env":{"OLD":"x"}}"#,
        );

        let input: Variables = vec![Variable {
            key: "KEY1".to_string(),
            required: true,
            value: Some("value1".to_string()),
            ..Default::default()
        }]
        .into();

        let output = ClaudeOutput::new(Some(path.clone()));
        let result = output.format(input).unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["other"], "keep");
        assert_eq!(parsed["env"]["KEY1"], "value1");
        assert!(parsed["env"].get("OLD").is_none());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_claude_load_existing() {
        let path = write_temp(
            "claude_load.json",
            r#"{"other":"keep","env":{"KEY1":"value1","KEY2":"val\"ue2"}}"#,
        );

        let output = ClaudeOutput::new(Some(path.clone()));
        let result = output.load_existing(File::open(&path).unwrap()).unwrap();

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

    #[test]
    fn test_claude_load_existing_no_env_key() {
        let path = write_temp("claude_load_no_env.json", r#"{"other":"keep"}"#);

        let output = ClaudeOutput::new(Some(path.clone()));
        let result = output.load_existing(File::open(&path).unwrap()).unwrap();

        assert!(result.is_empty());

        let _ = fs::remove_file(&path);
    }
}
