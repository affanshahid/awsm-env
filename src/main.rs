use std::{
    borrow::Cow,
    fs::{self, File},
    io::{self, Write},
};

use anyhow::{Context, Result};

use awsm_env::{
    ClaudeOutput, CodexOutput, EnvOutput, JsonOutput, Output, ShellOutput,
    cli::{Args, Format},
};
use clap::Parser;

// #[tokio::main]
// async fn main() -> Result<()> {
//     let args = Args::parse();

//     let extra_vars = args.vars();
//     let placeholders = args.placeholders();

//     let input = fs::read_to_string(args.spec).context("Failed to read specification file")?;
//     let mut input_entries = parse(&input).context("Failed to parse file")?;

//     if args.no_defaults {
//         for entry in input_entries.iter_mut() {
//             entry.value = None;
//         }
//     }

//     let outputter: Box<dyn Output> = match args.format {
//         Format::Env => Box::new(EnvOutput),
//         Format::Shell => Box::new(ShellOutput),
//         Format::Json => Box::new(JsonOutput),
//         Format::Claude => Box::new(ClaudeOutput::new(args.output.clone())),
//         Format::Codex => Box::new(CodexOutput::new(args.output.clone())),
//     };

//     let output_entries = process_entries(input_entries, &vars, &placeholders)
//         .await
//         .context("Failed to fetch secrets")?;

//     let existing = match args.output {
//         Some(ref out) if out.try_exists().context("Failed to check output file")? => {
//             let file = File::open(out).context("Failed to open existing output file")?;
//             Some(
//                 outputter
//                     .load_existing(file)
//                     .context("Failed to load values from existing output file")?,
//             )
//         }
//         _ => None,
//     };

//     let merged_entries = if let Some(existing) = existing {
//         merge(output_entries, existing, args.merge_mode)
//     } else {
//         output_entries
//             .into_iter()
//             .map(|(k, v)| (Cow::Borrowed(k), v))
//             .collect()
//     };

//     let output = outputter
//         .format(&merged_entries)
//         .context("Failed to format output")?;

//     match args.output {
//         Some(path) => {
//             if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
//                 fs::create_dir_all(parent).context("Failed to create parent directory")?;
//             }
//             fs::write(path, output.as_bytes()).context("writing to file")?
//         }
//         None => io::stdout()
//             .write_all(output.as_bytes())
//             .context("writing to file")?,
//     };

//     Ok(())
// }

fn main() {}
