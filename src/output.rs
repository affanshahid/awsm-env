use std::borrow::Cow;

use indexmap::IndexMap;
use tokio::{fs::File, io::AsyncReadExt};

use crate::{
    error::{Error, IoError, SerdeError},
    parse,
};

/// By implementing `Output` a type provides a way to format an [`IndexMap`]
/// of key value pairs and to load existing values back from a file in that format.
pub trait Output {
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> String;
    #[allow(async_fn_in_trait)]
    async fn load_existing(&self, file: File) -> Result<IndexMap<String, String>, Error>;
}

/// Formats environment entries into `.env` format using [`EnvOutput::format`]
pub struct EnvOutput;

impl Output for EnvOutput {
    /// Formats environment entries into `.env` format
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> String {
        let mut output = String::new();

        for (key, value) in entries {
            output.push_str(&format!(
                "{}={}\n",
                key,
                serde_json::to_string(&value).expect("should be able to JSONify string")
            ));
        }

        output
    }

    /// Loads existing environment entries from a file in `.env` format
    async fn load_existing(&self, mut file: File) -> Result<IndexMap<String, String>, Error> {
        let mut input = String::new();
        file.read_to_string(&mut input)
            .await
            .map_err(IoError::from)?;

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
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> String {
        let mut output = String::new();

        for (key, value) in entries {
            output.push_str(&format!(
                "export {}={}\n",
                key,
                serde_json::to_string(&value).expect("should be able to JSONify string")
            ));
        }

        output
    }

    /// Loads existing environment entries from a file in shell variable export command format
    async fn load_existing(&self, mut file: File) -> Result<IndexMap<String, String>, Error> {
        let mut input = String::new();
        file.read_to_string(&mut input)
            .await
            .map_err(IoError::from)?;

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
    fn format(&self, entries: &IndexMap<Cow<str>, Cow<str>>) -> String {
        serde_json::to_string(entries).expect("IndexMap should be serialized to JSON") + "\n"
    }

    /// Loads existing environment entries from a JSON file
    async fn load_existing(&self, mut file: File) -> Result<IndexMap<String, String>, Error> {
        let mut input = String::new();
        file.read_to_string(&mut input)
            .await
            .map_err(IoError::from)?;
        let obj = serde_json::from_str(&input).map_err(SerdeError::from)?;

        Ok(obj)
    }
}

#[cfg(test)]
mod tests {
    use tokio::fs;

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
        let result = output.format(&input);
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
        let result = output.format(&input);
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
        let result = output.format(&input);

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            serde_json::to_value(input).unwrap()
        )
    }

    async fn write_temp(name: &str, contents: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("awsm_env_test_{}", name));
        fs::write(&path, contents).await.unwrap();
        path
    }

    #[tokio::test]
    async fn test_env_load_existing() {
        let path = write_temp("env_load.env", "KEY1=\"value1\"\nKEY2=\"val\\\"ue2\"\n").await;

        let result = EnvOutput
            .load_existing(File::open(&path).await.unwrap())
            .await
            .unwrap();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn test_shell_load_existing() {
        let path = write_temp(
            "shell_load.sh",
            "export KEY1=\"value1\"\nexport KEY2=\"val\\\"ue2\"\n",
        )
        .await;

        let result = ShellOutput
            .load_existing(File::open(&path).await.unwrap())
            .await
            .unwrap();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path).await;
    }

    #[tokio::test]
    async fn test_json_load_existing() {
        let path = write_temp(
            "json_load.json",
            "{\"KEY1\":\"value1\",\"KEY2\":\"val\\\"ue2\"}\n",
        )
        .await;

        let result = JsonOutput
            .load_existing(File::open(&path).await.unwrap())
            .await
            .unwrap();

        let mut expected = IndexMap::new();
        expected.insert("KEY1".to_string(), "value1".to_string());
        expected.insert("KEY2".to_string(), "val\"ue2".to_string());
        assert_eq!(result, expected);

        let _ = fs::remove_file(&path).await;
    }
}
