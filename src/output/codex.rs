use std::{
    fs::{self, File},
    io,
    path::PathBuf,
};

use indexmap::IndexMap;
use toml::Table;

use crate::{output::Output, variable::Variables};

use anyhow::Result;

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

    use crate::variable::Variable;

    use super::*;

    fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("awsm_env_test_{}", name));
        fs::write(&path, contents).unwrap();
        path
    }

    #[test]
    fn test_codex_output() {
        let path = std::env::temp_dir().join("awsm_env_test_codex_new.toml");
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

        let input: Variables = vec![Variable {
            key: "KEY1".to_string(),
            required: true,
            value: Some("value1".to_string()),
            ..Default::default()
        }]
        .into();

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
    fn test_codex_load_existing_no_env_key() {
        let path = write_temp("codex_load_no_env.toml", "model = \"gpt-5\"\n");

        let output = CodexOutput::new(Some(path.clone()));
        let result = output.load_existing(File::open(&path).unwrap()).unwrap();

        assert!(result.is_empty());

        let _ = fs::remove_file(&path);
    }
}
