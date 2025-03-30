use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("unable to parse input:\n{0}")]
    ParsingError(String),
}
