use std::{
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
};

use awsm_env::{EnvFormatter, Formatter, JsonFormatter, ShellFormatter, parse, process_entries};
use clap::{Parser, ValueEnum};
use indexmap::IndexMap;
use tokio::fs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the spec file
    #[arg(default_value = ".env.example")]
    spec: PathBuf,

    /// Output format
    #[arg(long, short, value_enum, default_value = "env")]
    format: Format,

    /// Path of a file to write the output to instead of writing to stdout
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Variable definitions of the form `KEY=value` to add or override keys
    /// in the output
    #[arg(long = "var", short, value_parser = parse_key_val)]
    vars: Option<Vec<(String, String)>>,

    /// Placeholder definitions of the form `KEY=value` to be used in secret names
    #[arg(long, short, value_parser = parse_key_val)]
    placeholders: Option<Vec<(String, String)>>,
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

#[derive(Clone, ValueEnum)]
enum Format {
    Env,
    Shell,
    Json,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let vars: IndexMap<String, String> = args
        .vars
        .unwrap_or(Vec::with_capacity(0))
        .into_iter()
        .collect();

    let placeholders: HashMap<String, String> = args
        .placeholders
        .unwrap_or(Vec::with_capacity(0))
        .into_iter()
        .collect();

    let input = match fs::read_to_string(args.spec).await {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Error reading file: {}", err);
            return;
        }
    };

    let input_entries = match parse(&input) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Error parsing file: {}", err);
            return;
        }
    };

    let output_entries = match process_entries(input_entries, &vars, &placeholders).await {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Error fetching secrets: {}", err);
            return;
        }
    };

    let output = match args.format {
        Format::Env => EnvFormatter::new().format(&output_entries),
        Format::Shell => ShellFormatter::new().format(&output_entries),
        Format::Json => JsonFormatter::new().format(&output_entries),
    };

    let result = match args.output {
        Some(path) => fs::write(path, output.as_bytes()).await,
        None => io::stdout().write_all(output.as_bytes()),
    };

    match result {
        Ok(_) => (),
        Err(err) => {
            eprintln!("Error writing output: {}", err);
        }
    };
}
