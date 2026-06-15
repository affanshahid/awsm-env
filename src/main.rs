use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use awsm_env::{
    EnvOutput, JsonOutput, MergeMode, Output, ShellOutput, merge, parse, process_entries,
};
use clap::{Parser, ValueEnum};
use indexmap::IndexMap;
use tokio::fs::{self, File};

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
    #[arg(long = "placeholder", short, value_parser = parse_key_val)]
    placeholders: Option<Vec<(String, String)>>,

    /// Don't use defaults from the spec file
    #[arg(long)]
    no_defaults: bool,

    /// Merge mode to use when merging with existing output file. Defaults to `overwrite`.
    #[arg(long, short, value_enum, default_value_t = MergeMode::Overwrite)]
    merge_mode: MergeMode,
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
async fn main() -> ExitCode {
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
            return ExitCode::FAILURE;
        }
    };

    let mut input_entries = match parse(&input) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Error parsing file: {}", err);
            return ExitCode::FAILURE;
        }
    };

    if args.no_defaults {
        for entry in input_entries.iter_mut() {
            entry.value = None;
        }
    }

    let output_entries = match process_entries(input_entries, &vars, &placeholders).await {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Error fetching secrets: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let existing = if let Some(ref out) = args.output {
        match out.try_exists() {
            Ok(true) => match File::open(out).await {
                Ok(file) => match match args.format {
                    Format::Env => EnvOutput.load_existing(file).await,
                    Format::Shell => ShellOutput.load_existing(file).await,
                    Format::Json => JsonOutput.load_existing(file).await,
                } {
                    Ok(existing) => Some(existing),
                    Err(err) => {
                        eprintln!("Error loading existing output file: {}", err);
                        return ExitCode::FAILURE;
                    }
                },
                Err(err) => {
                    eprintln!("Error opening existing output file: {}", err);
                    return ExitCode::FAILURE;
                }
            },
            Ok(false) => None,
            Err(err) => {
                eprintln!("Error checking if output file exists: {}", err);
                return ExitCode::FAILURE;
            }
        }
    } else {
        None
    };

    let merged_entries = if let Some(existing) = existing {
        merge(output_entries, existing, args.merge_mode)
    } else {
        output_entries
            .into_iter()
            .map(|(k, v)| (Cow::Borrowed(k), v))
            .collect()
    };

    let output = match args.format {
        Format::Env => EnvOutput.format(&merged_entries),
        Format::Shell => ShellOutput.format(&merged_entries),
        Format::Json => JsonOutput.format(&merged_entries),
    };

    let result = match args.output {
        Some(path) => fs::write(path, output.as_bytes()).await,
        None => io::stdout().write_all(output.as_bytes()),
    };

    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Error writing output: {}", err);
            ExitCode::FAILURE
        }
    }
}
