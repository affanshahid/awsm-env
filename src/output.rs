use std::{
    borrow::Cow,
    fs::{self, File},
    io,
    path::PathBuf,
};

use indexmap::IndexMap;
use serde_json::{Map, Value};
use toml::Table;

use crate::error::{Error, IoError, SerdeError, TomlDeError, TomlSerError};

/// By implementing `Output` a type provides a way to format an [`IndexMap`]
/// of key value pairs and to load existing values back from a file in that format.
pub trait Output {
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> Result<String, Error>;
    fn load_existing(&self, file: File) -> Result<IndexMap<String, String>, Error>;
}

/// Formats environment entries into `.env` format using [`EnvOutput::format`]
pub struct EnvOutput;

impl Output for EnvOutput {
    /// Formats environment entries into `.env` format
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> Result<String, Error> {
        let mut output = String::new();

        for (key, value) in entries {
            output.push_str(&format!(
                "{}={}\n",
                key,
                serde_json::to_string(&value).expect("should be able to JSONify string")
            ));
        }

        Ok(output)
    }

    /// Loads existing environment entries from a file in `.env` format
    fn load_existing(&self, file: File) -> Result<IndexMap<String, String>, Error> {
        let input = io::read_to_string(file).map_err(IoError::from)?;
        let input_entries = parse(&input)?;
        Ok(input_entries
            .into_iter()
            .filter_map(|e| e.value.map(|v| (e.key.to_string(), v.to_string())))
            .collect())
    }
}

/// Formats environment entries into shell variable export commands using [`ShellOutput::format`]
pub struct ShellOutput;

impl Output for ShellOutput {
    /// Formats environment entries into shell variable export commands
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> Result<String, Error> {
        let mut output = String::new();

        for (key, value) in entries {
            output.push_str(&format!(
                "export {}={}\n",
                key,
                serde_json::to_string(&value).expect("should be able to JSONify string")
            ));
        }

        Ok(output)
    }

    /// Loads existing environment entries from a file in shell variable export command format
    fn load_existing(&self, file: File) -> Result<IndexMap<String, String>, Error> {
        let input = io::read_to_string(file).map_err(IoError::from)?;

        let input_entries = parse(&input)?;
        Ok(input_entries
            .into_iter()
            .filter_map(|e| e.value.map(|v| (e.key.to_string(), v.to_string())))
            .collect())
    }
}

/// Formats environment entries into JSON using [`JsonOutput::format`]
pub struct JsonOutput;

impl Output for JsonOutput {
    /// Formats environment entries into JSON of the form `{"KEY": "value"}`
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> Result<String, Error> {
        Ok(serde_json::to_string(entries).expect("IndexMap should be serialized to JSON") + "\n")
    }

    /// Loads existing environment entries from a JSON file
    fn load_existing(&self, file: File) -> Result<IndexMap<String, String>, Error> {
        let input = io::read_to_string(file).map_err(IoError::from)?;
        let obj = serde_json::from_str(&input).map_err(SerdeError::from)?;

        Ok(obj)
    }
}

/// Formats environment entries into Claude Code's settings file format using [`ClaudeOutput::format`]
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
    /// Formats environment entries into Claude Code's settings file format of the form `{"env": {"KEY": "value"}}`
    /// Preserves other existing settings
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> Result<String, Error> {
        let mut map = match self.path.try_exists().map_err(IoError::from)? {
            true => {
                let input = fs::read_to_string(&self.path).map_err(IoError::from)?;
                let obj: Value = serde_json::from_str(&input).map_err(SerdeError::from)?;
                serde_json::from_value(obj).map_err(SerdeError::from)?
            }
            false => Map::new(),
        };
        map.insert(
            "env".to_string(),
            serde_json::to_value(entries).map_err(SerdeError::from)?,
        );

        Ok(serde_json::to_string_pretty(&map).map_err(SerdeError::from)?)
    }

    /// Loads existing environment entries from a Claude Code settings file
    fn load_existing(&self, file: File) -> Result<IndexMap<String, String>, Error> {
        let input = io::read_to_string(file).map_err(IoError::from)?;
        let mut obj: Value = serde_json::from_str(&input).map_err(SerdeError::from)?;
        let env = obj
            .as_object_mut()
            .and_then(|o| o.remove("env"))
            .unwrap_or(Value::Object(Map::new()));
        let env = serde_json::from_value(env).map_err(SerdeError::from)?;

        Ok(env)
    }
}

/// Formats environment entries into Codex CLI's `config.toml` format using [`CodexOutput::format`]
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
    /// Formats environment entries under `[shell_environment_policy.set]` in Codex's
    /// `config.toml`. Preserves other existing settings; replaces the whole `set` table.
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> Result<String, Error> {
        let mut map: Table = match self.path.try_exists().map_err(IoError::from)? {
            true => {
                let input = fs::read_to_string(&self.path).map_err(IoError::from)?;
                toml::from_str(&input).map_err(TomlDeError::from)?
            }
            false => Table::new(),
        };

        let mut policy: Table = map
            .remove("shell_environment_policy")
            .and_then(|v| v.try_into().ok())
            .unwrap_or_default();

        policy.insert(
            "set".to_string(),
            toml::Value::try_from(entries).map_err(TomlSerError::from)?,
        );
        map.insert("shell_environment_policy".to_string(), policy.into());

        Ok(toml::to_string_pretty(&map).map_err(TomlSerError::from)?)
    }

    /// Loads existing environment entries from a Codex `config.toml` file
    fn load_existing(&self, file: File) -> Result<IndexMap<String, String>, Error> {
        let input = io::read_to_string(file).map_err(IoError::from)?;
        let mut obj: Table = toml::from_str(&input).map_err(TomlDeError::from)?;
        let set = obj
            .remove("shell_environment_policy")
            .and_then(|v| v.try_into::<Table>().ok())
            .and_then(|mut t| t.remove("set"))
            .map(|v| v.try_into())
            .transpose()
            .map_err(TomlDeError::from)?
            .unwrap_or_default();

        Ok(set)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn test_env_output() {
        let mut input = IndexMap::new();
        input.insert(
            Cow::Owned("KEY1".to_string()),
            Cow::Owned("value1".to_string()),
        );
        input.insert(
            Cow::Owned("KEY2".to_string()),
            Cow::Owned("val\"ue2".to_string()),
        );

        let output = EnvOutput;
        let result = output.format(&input).unwrap();
        assert_eq!(result, "KEY1=\"value1\"\nKEY2=\"val\\\"ue2\"\n")
    }

    #[test]
    fn test_shell_output() {
        let mut input = IndexMap::new();
        input.insert(
            Cow::Owned("KEY1".to_string()),
            Cow::Owned("value1".to_string()),
        );
        input.insert(
            Cow::Owned("KEY2".to_string()),
            Cow::Owned("val\"ue2".to_string()),
        );

        let output = ShellOutput;
        let result = output.format(&input).unwrap();
        assert_eq!(
            result,
            "export KEY1=\"value1\"\nexport KEY2=\"val\\\"ue2\"\n"
        )
    }

    #[test]
    fn test_json_output() {
        let mut input = IndexMap::new();
        input.insert(
            Cow::Owned("KEY1".to_string()),
            Cow::Owned("value1".to_string()),
        );
        input.insert(
            Cow::Owned("KEY2".to_string()),
            Cow::Owned("val\"ue2".to_string()),
        );

        let output = JsonOutput;
        let result = output.format(&input).unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            serde_json::to_value(input).unwrap()
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

        let mut input = IndexMap::new();
        input.insert(
            Cow::Owned("KEY1".to_string()),
            Cow::Owned("value1".to_string()),
        );
        input.insert(
            Cow::Owned("KEY2".to_string()),
            Cow::Owned("val\"ue2".to_string()),
        );

        let output = ClaudeOutput::new(Some(path.clone()));
        let result = output.format(&input).unwrap();

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

        let mut input = IndexMap::new();
        input.insert(
            Cow::Owned("KEY1".to_string()),
            Cow::Owned("value1".to_string()),
        );

        let output = ClaudeOutput::new(Some(path.clone()));
        let result = output.format(&input).unwrap();

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

        let mut input = IndexMap::new();
        input.insert(
            Cow::Owned("KEY1".to_string()),
            Cow::Owned("value1".to_string()),
        );
        input.insert(
            Cow::Owned("KEY2".to_string()),
            Cow::Owned("val\"ue2".to_string()),
        );

        let output = CodexOutput::new(Some(path.clone()));
        let result = output.format(&input).unwrap();

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

        let mut input = IndexMap::new();
        input.insert(
            Cow::Owned("KEY1".to_string()),
            Cow::Owned("value1".to_string()),
        );

        let output = CodexOutput::new(Some(path.clone()));
        let result = output.format(&input).unwrap();

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
