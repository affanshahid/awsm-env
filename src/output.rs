use std::{
    fs::{self, File},
    io,
    path::PathBuf,
};

use indexmap::IndexMap;
use serde_json::{Map, Value};
use toml::Table;

use crate::{parser::EnvParser, variable::Variables};

use anyhow::Result;

/// By implementing `Output` a type provides a way to format an [`IndexMap`]
/// of key value pairs and to load existing values back from a file in that format.
pub trait Output {
    fn format(&self, variables: Variables) -> Result<String>;
    fn load_existing(&self, file: File) -> Result<Variables>;
}

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
                serde_json::to_string(&var.value)?
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
                serde_json::to_string(&var.value)?
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

/// Formats environment variables into Codex CLI's `config.toml` format using [`CodexOutput::format`]
pub struct CodexOutput {
    path: PathBuf,
}

impl CodexOutput {
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path: path.unwrap_or(PathBuf::from(".codex/config.toml")),
        }
    }
}

impl Output for CodexOutput {
    /// Formats environment variables under `[shell_environment_policy.set]` in Codex's
    /// `config.toml`. Preserves other existing settings; replaces the whole `set` table.
    fn format(&self, variables: Variables) -> Result<String> {
        let mut map: Table = match self.path.try_exists()? {
            true => {
                let input = fs::read_to_string(&self.path)?;
                toml::from_str(&input)?
            }
            false => Table::new(),
        };

        let mut policy: Table = map
            .remove("shell_environment_policy")
            .and_then(|v| v.try_into().ok())
            .unwrap_or_default();

        let env_map: IndexMap<_, _> = variables.into();

        policy.insert("set".to_string(), toml::Value::try_from(env_map)?);
        map.insert("shell_environment_policy".to_string(), policy.into());

        Ok(toml::to_string_pretty(&map)?)
    }

    /// Loads existing environment variables from a Codex `config.toml` file
    fn load_existing(&self, file: File) -> Result<Variables> {
        let input = io::read_to_string(file)?;
        let mut obj: Table = toml::from_str(&input)?;
        let set: IndexMap<_, _> = obj
            .remove("shell_environment_policy")
            .and_then(|v| v.try_into::<Table>().ok())
            .and_then(|mut t| t.remove("set"))
            .map(|v| v.try_into())
            .transpose()?
            .unwrap_or_default();

        Ok(set.into())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    /// Builds a [`Variables`] from key/value pairs (each becomes a variable with a `value`).
    fn variables(pairs: &[(&str, &str)]) -> Variables {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<IndexMap<String, String>>()
            .into()
    }

    #[test]
    fn test_env_output() {
        let input = variables(&[("KEY1", "value1"), ("KEY2", "val\"ue2")]);

        let output = EnvOutput;
        let result = output.format(input).unwrap();
        assert_eq!(result, "KEY1=\"value1\"\nKEY2=\"val\\\"ue2\"\n")
    }

    #[test]
    fn test_shell_output() {
        let input = variables(&[("KEY1", "value1"), ("KEY2", "val\"ue2")]);

        let output = ShellOutput;
        let result = output.format(input).unwrap();
        assert_eq!(
            result,
            "export KEY1=\"value1\"\nexport KEY2=\"val\\\"ue2\"\n"
        )
    }

    #[test]
    fn test_json_output() {
        let pairs = [("KEY1", "value1"), ("KEY2", "val\"ue2")];

        let output = JsonOutput;
        let result = output.format(variables(&pairs)).unwrap();

        let expected: IndexMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            serde_json::to_value(expected).unwrap()
        )
    }

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("awsm_env_test_{}", name));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn test_env_load_existing() {
        let path = write_temp("env_load.env", "KEY1=\"value1\"\nKEY2=\"val\\\"ue2\"\n");

        let result = EnvOutput.load_existing(File::open(&path).unwrap()).unwrap();
        let result: IndexMap<String, String> = result.into();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path);
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
        let result: IndexMap<String, String> = result.into();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path);
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
        let result: IndexMap<String, String> = result.into();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_claude_output() {
        let path = std::env::temp_dir().join("awsm_env_test_claude_new.json");
        let _ = fs::remove_file(&path);

        let input = variables(&[("KEY1", "value1"), ("KEY2", "val\"ue2")]);

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

        let input = variables(&[("KEY1", "value1")]);

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
        let result: IndexMap<String, String> = result.into();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
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

    #[test]
    fn test_codex_output() {
        let path = std::env::temp_dir().join("awsm_env_test_codex_new.toml");
        let _ = fs::remove_file(&path);

        let input = variables(&[("KEY1", "value1"), ("KEY2", "val\"ue2")]);

        let output = CodexOutput::new(Some(path.clone()));
        let result = output.format(input).unwrap();

        let parsed: toml::Table = toml::from_str(&result).unwrap();
        let set = parsed["shell_environment_policy"]["set"]
            .as_table()
            .unwrap();
        assert_eq!(set["KEY1"].as_str().unwrap(), "value1");
        assert_eq!(set["KEY2"].as_str().unwrap(), "val\"ue2");
    }

    #[test]
    fn test_codex_output_preserves_other_settings() {
        let path = write_temp(
            "codex_preserve.toml",
            "model = \"gpt-5\"\n\n[shell_environment_policy]\ninherit = \"core\"\n\n[shell_environment_policy.set]\nOLD = \"x\"\n",
        );

        let input = variables(&[("KEY1", "value1")]);

        let output = CodexOutput::new(Some(path.clone()));
        let result = output.format(input).unwrap();

        let parsed: toml::Table = toml::from_str(&result).unwrap();
        assert_eq!(parsed["model"].as_str().unwrap(), "gpt-5");
        assert_eq!(
            parsed["shell_environment_policy"]["inherit"]
                .as_str()
                .unwrap(),
            "core"
        );
        let set = parsed["shell_environment_policy"]["set"]
            .as_table()
            .unwrap();
        assert_eq!(set["KEY1"].as_str().unwrap(), "value1");
        assert!(set.get("OLD").is_none());

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_codex_load_existing() {
        let path = write_temp(
            "codex_load.toml",
            "[shell_environment_policy.set]\nKEY1 = \"value1\"\nKEY2 = \"val\\\"ue2\"\n",
        );

        let output = CodexOutput::new(Some(path.clone()));
        let result = output.load_existing(File::open(&path).unwrap()).unwrap();
        let result: IndexMap<String, String> = result.into();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn test_codex_load_existing_no_env_key() {
        let path = write_temp("codex_load_no_env.toml", "model = \"gpt-5\"\n");

        let output = CodexOutput::new(Some(path.clone()));
        let result = output.load_existing(File::open(&path).unwrap()).unwrap();

        assert!(result.is_empty());

        let _ = fs::remove_file(&path);
    }
}
