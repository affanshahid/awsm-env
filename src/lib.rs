//! Awsm Env
//!
//! A lightweight utility for syncing AWS Secrets Manager secrets to environment variables.

mod error;
mod parser;

pub use parser::{EnvEntries, EnvEntry, parse};
