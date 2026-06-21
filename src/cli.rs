use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use indexmap::IndexMap;

#[derive(Clone, ValueEnum)]
pub enum Format {
    Env,
    Shell,
    Json,
    Claude,
    Codex,
}

#[derive(ValueEnum, Clone, Eq, PartialEq)]
pub enum MergeMode {
    Overwrite,
    Fallback,
    Override,
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Path to the spec file
    #[arg(default_value = ".env.example")]
    pub spec: PathBuf,

    /// Output format
    #[arg(long, short, value_enum, default_value = "env")]
    pub format: Format,

    /// Path of a file to write the output to instead of writing to stdout
    #[arg(long, short)]
    pub output: Option<PathBuf>,

    /// Variable definitions of the form `KEY=value` to add or override keys
    /// in the output
    #[arg(long = "var", short, value_parser = parse_key_val)]
    pub vars: Option<Vec<(String, String)>>,

    /// Placeholder definitions of the form `KEY=value` to be used in secret names
    #[arg(long = "placeholder", short, value_parser = parse_key_val)]
    pub placeholders: Option<Vec<(String, String)>>,

    /// Don't use defaults from the spec file
    #[arg(long)]
    pub no_defaults: bool,

    /// Merge mode to use when merging with existing output file. Defaults to `overwrite`.
    #[arg(long, short, value_enum, default_value_t = MergeMode::Overwrite)]
    pub merge_mode: MergeMode,
}

impl Args {
    pub fn placeholders(&self) -> IndexMap<String, String> {
        self.placeholders.iter().flatten().cloned().collect()
    }

    pub fn vars(&self) -> IndexMap<String, String> {
        self.vars.iter().flatten().cloned().collect()
    }
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let mut split = s.split("=");
    let key = split
        .next()
        .ok_or(format!("Key value pairs should be of the form key=value"))?;
    let value = split
        .next()
        .ok_or(format!("Key value pairs should be of the form key=value",))?;

    Ok((key.to_string(), value.to_owned()))
}
