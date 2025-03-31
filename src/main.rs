use std::{
    io::{self, Write},
    path::PathBuf,
};

use awsm_env::{EnvFormatter, Formatter, JsonFormatter, ShellFormatter, parse, process_entries};
use clap::{Parser, ValueEnum};
use tokio::fs;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(default_value = ".env.example")]
    spec: PathBuf,

    #[arg(long, short, value_enum, default_value = "env")]
    format: Format,

    #[arg(long, short)]
    output: Option<PathBuf>,
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

    let input = match fs::read_to_string(args.spec).await {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Error reading file: {}", err);
            return;
        }
    };

    let mut input_entries = match parse(&input) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Error parsing file: {}", err);
            return;
        }
    };

    let output_entries = match process_entries(&mut input_entries).await {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Error fetching secrets: {}", err);
            return;
        }
    };

    let output = match args.format {
        Format::Env => EnvFormatter::new().format(output_entries),
        Format::Shell => ShellFormatter::new().format(output_entries),
        Format::Json => JsonFormatter::new().format(output_entries),
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
