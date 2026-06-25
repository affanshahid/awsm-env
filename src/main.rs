use std::{
    fs::{self, File},
    io::{self, Write},
};

use anyhow::{Context, Result};

use awsm_env::{
    cli::{Args, Format},
    output::{ClaudeOutput, CodexOutput, EnvOutput, JsonOutput, Output, ShellOutput},
    parser::EnvParser,
    resolve::{merge, resolve},
};
use clap::Parser;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let placeholders = args.placeholders();
    let extra_vars = args.vars();

    let input = fs::read_to_string(args.spec).context("Failed to read specification file")?;
    let mut variables = EnvParser::parse_variables(&input).context("Failed to parse file")?;

    if args.no_defaults {
        variables.iter_mut().for_each(|var| var.drop_default());
    }

    resolve(&mut variables, placeholders)
        .await
        .context("Failed to fetch secrets")?;

    variables.merge(extra_vars);

    let outputter: Box<dyn Output> = match args.format {
        Format::Env => Box::new(EnvOutput),
        Format::Shell => Box::new(ShellOutput),
        Format::Json => Box::new(JsonOutput),
        Format::Claude => Box::new(ClaudeOutput::new(args.output.clone())),
        Format::Codex => Box::new(CodexOutput::new(args.output.clone())),
    };

    variables = match args.output {
        Some(ref out) if out.try_exists().context("Failed to check output file")? => {
            let file = File::open(out).context("Failed to open existing output file")?;
            let existing = outputter
                .load_existing(file)
                .context("Failed to load values from existing output file")?;

            merge(variables, existing, args.merge_mode)
        }
        _ => variables,
    };

    variables.drop_empty();

    let output = outputter
        .format(variables)
        .context("Failed to format output")?;

    match args.output {
        Some(path) => {
            if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                fs::create_dir_all(parent).context("Failed to create parent directory")?;
            }
            fs::write(path, output.as_bytes()).context("writing to file")?
        }
        None => io::stdout()
            .write_all(output.as_bytes())
            .context("writing to file")?,
    };

    Ok(())
}
