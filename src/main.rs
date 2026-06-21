use std::{
    borrow::Cow,
    fs::{self, File},
    io::{self, Write},
    process::ExitCode,
};

use awsm_env::{
    ClaudeOutput, CodexOutput, EnvOutput, JsonOutput, Output, ShellOutput,
    cli::{Args, Format},
    merge, parse, process_entries,
};
use clap::Parser;

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();

    let vars = args.vars();
    let placeholders = args.placeholders();

    let input = match fs::read_to_string(args.spec) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Error reading specification file: {}", err);
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

    let outputter: Box<dyn Output> = match args.format {
        Format::Env => Box::new(EnvOutput),
        Format::Shell => Box::new(ShellOutput),
        Format::Json => Box::new(JsonOutput),
        Format::Claude => Box::new(ClaudeOutput::new(args.output.clone())),
        Format::Codex => Box::new(CodexOutput::new(args.output.clone())),
    };

    let output_entries = match process_entries(input_entries, &vars, &placeholders).await {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Error fetching secrets: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let existing = if let Some(ref out) = args.output {
        match out.try_exists() {
            Ok(true) => match File::open(out) {
                Ok(file) => match outputter.load_existing(file) {
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

    let output = match outputter.format(&merged_entries) {
        Ok(output) => output,
        Err(err) => {
            eprintln!("Error formatting output: {}", err);
            return ExitCode::FAILURE;
        }
    };

    let result = match args.output {
        Some(path) => {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("Error creating output directory: {}", err);
                    return ExitCode::FAILURE;
                }
            }
            fs::write(path, output.as_bytes())
        }
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
