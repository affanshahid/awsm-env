use std::borrow::Cow;

use indexmap::IndexMap;

/// By implementing `Formatter` a type provides a way to
/// format an [`IndexMap`] of key value pairs
pub trait Formatter {
    fn format(&self, entries: &IndexMap<&str, Cow<str>>) -> String;
}

/// Formats environment entries into `.env` format using [`EnvOutput::format`]
pub struct EnvFormatter {}

impl EnvFormatter {
    pub fn new() -> Self {
        EnvFormatter {}
    }
}

impl Formatter for EnvFormatter {
    /// Formats environment entries into `.env` format
    fn format(&self, entries: &IndexMap<&str, Cow<str>>) -> String {
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
}

/// Formats environment entries into shell variable export commands using [`ShellOutput::format`]
pub struct ShellFormatter {}

impl ShellFormatter {
    pub fn new() -> Self {
        ShellFormatter {}
    }
}

impl Formatter for ShellFormatter {
    /// Formats environment entries into shell variable export commands
    fn format(&self, entries: &IndexMap<&str, Cow<str>>) -> String {
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
}

/// Formats environment entries into JSON using [`JsonOutput::format`]
pub struct JsonFormatter {}

impl JsonFormatter {
    pub fn new() -> Self {
        JsonFormatter {}
    }
}

impl Formatter for JsonFormatter {
    /// Formats environment entries into JSON of the form `{"KEY": "value"}`
    fn format(&self, entries: &IndexMap<&str, Cow<str>>) -> String {
        serde_json::to_string(entries).expect("IndexMap should be serialized to JSON") + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_output() {
        let mut input = IndexMap::new();
        input.insert("KEY1", Cow::Owned("value1".to_string()));
        input.insert("KEY2", Cow::Owned("val\"ue2".to_string()));

        let formatter = EnvFormatter::new();
        let result = formatter.format(&input);
        assert_eq!(result, "KEY1=\"value1\"\nKEY2=\"val\\\"ue2\"\n")
    }

    #[test]
    fn test_shell_output() {
        let mut input = IndexMap::new();
        input.insert("KEY1", Cow::Owned("value1".to_string()));
        input.insert("KEY2", Cow::Owned("val\"ue2".to_string()));

        let formatter = ShellFormatter::new();
        let result = formatter.format(&input);
        assert_eq!(
            result,
            "export KEY1=\"value1\"\nexport KEY2=\"val\\\"ue2\"\n"
        )
    }

    #[test]
    fn test_json_output() {
        let mut input = IndexMap::new();
        input.insert("KEY1", Cow::Owned("value1".to_string()));
        input.insert("KEY2", Cow::Owned("val\"ue2".to_string()));

        let formatter = JsonFormatter::new();
        let result = formatter.format(&input);

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            serde_json::to_value(input).unwrap()
        )
    }
}
