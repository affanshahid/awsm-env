mod claude;
mod codex;
mod env;
mod json;
mod shell;

pub use claude::ClaudeOutput;
pub use codex::CodexOutput;
pub use env::EnvOutput;
pub use json::JsonOutput;
pub use shell::ShellOutput;

use std::fs::File;

use crate::variable::Variables;

use anyhow::Result;

/// By implementing `Output` a type provides a way to format [`Variables`]
/// and to load existing values back from a file in that format.
pub trait Output {
    fn format(&self, variables: Variables) -> Result<String>;
    fn load_existing(&self, file: File) -> Result<Variables>;
}
