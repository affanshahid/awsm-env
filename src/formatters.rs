use std::collections::HashMap;

pub struct OutputEntry<'a>(pub &'a str, pub &'a str);

/// By implementing `Formatter` a type provides a way to
/// format [`OutputEntry`]s
pub trait Formatter<'a, I: IntoIterator<Item = OutputEntry<'a>>> {
    fn format(&self, entries: I) -> String;
}

/// Formats environment entries into `.env` format using [`EnvOutput::format`]
pub struct EnvFormatter {}

impl<'a, I: IntoIterator<Item = OutputEntry<'a>>> Formatter<'a, I> for EnvFormatter {
    /// Formats environment entries into `.env` format
    fn format(&self, entries: I) -> String {
        let mut output = String::new();

        for entry in entries {
            output.push_str(&format!("{}={}\n", entry.0, entry.1));
        }

        output
    }
}

/// Formats environment entries into shell variable export commands using [`ShellOutput::format`]
pub struct ShellFormatter {}

impl<'a, I: IntoIterator<Item = OutputEntry<'a>>> Formatter<'a, I> for ShellFormatter {
    /// Formats environment entries into shell variable export commands
    fn format(&self, entries: I) -> String {
        let mut output = String::new();

        for entry in entries {
            output.push_str(&format!("export {}={}\n", entry.0, entry.1));
        }

        output
    }
}

/// Formats environment entries into JSON using [`JsonOutput::format`]
pub struct JsonFormatter {}

impl<'a, I: IntoIterator<Item = OutputEntry<'a>>> Formatter<'a, I> for JsonFormatter {
    /// Formats environment entries into JSON of the form `{"KEY": "value"}`
    fn format(&self, entries: I) -> String {
        let mut output = HashMap::new();

        for entry in entries {
            output.insert(entry.0, entry.1);
        }

        serde_json::to_string(&output).expect("HashMap should be serialized to JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_output() {
        let input = vec![OutputEntry("KEY1", "value1"), OutputEntry("KEY2", "value2")];

        let formatter = EnvFormatter {};
        let result = formatter.format(input);
        assert_eq!(result, "KEY1=value1\nKEY2=value2\n")
    }

    #[test]
    fn test_shell_output() {
        let input = vec![OutputEntry("KEY1", "value1"), OutputEntry("KEY2", "value2")];

        let formatter = ShellFormatter {};
        let result = formatter.format(input);
        assert_eq!(result, "export KEY1=value1\nexport KEY2=value2\n")
    }

    #[test]
    fn test_json_output() {
        let input = vec![OutputEntry("KEY1", "value1"), OutputEntry("KEY2", "value2")];

        let formatter = JsonFormatter {};
        let result = formatter.format(input);

        let mut expected = HashMap::new();

        expected.insert("KEY1", "value1");
        expected.insert("KEY2", "value2");

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result).unwrap(),
            serde_json::to_value(expected).unwrap()
        )
    }
}
