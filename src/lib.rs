//! Awsm Env
//!
//! A lightweight utility for syncing AWS Secrets Manager secrets to environment variables.

pub mod cli;
pub mod output;
pub mod parser;
pub mod provider;
mod resolve;
mod variable;

pub use output::{ClaudeOutput, CodexOutput, EnvOutput, JsonOutput, Output, ShellOutput};
