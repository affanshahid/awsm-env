use aws_sdk_secretsmanager::{
    error::SdkError as SecretsManagerSdkError,
    operation::batch_get_secret_value::BatchGetSecretValueError,
    types::ApiErrorType as SecretsManagerApiErrorType,
};

use aws_sdk_ssm::{
    error::SdkError as ParameterStoreSdkError, operation::get_parameters::GetParametersError,
};

use std::io;
use thiserror::Error;

use crate::parser::Rule;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Unable to parse input:\n{0}")]
    ParsingError(#[from] pest::error::Error<Rule>),

    #[error("AWS SDK error: {0}")]
    AwsSmSdkError(#[from] AwsSmSdkError),

    #[error("AWS API error: {0}")]
    AwsSmApiError(#[from] AwsSmApiError),

    #[error("AWS SDK error: {0}")]
    AwsPsSdkError(#[from] AwsPsSdkError),

    #[error("Placeholder value missing for '{0}'")]
    PlaceholderMissing(String),

    #[error("Parameter not found: {0}")]
    ParameterNotFound(String),

    #[error("IO error: {0}")]
    IoError(#[from] IoError),

    #[error("Serialization / Deserialization error: {0}")]
    SerdeError(#[from] SerdeError),

    #[error("TOML deserialization error: {0}")]
    TomlDeError(#[from] TomlDeError),

    #[error("TOML serialization error: {0}")]
    TomlSerError(#[from] TomlSerError),
}

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsSmSdkError(#[from] SecretsManagerSdkError<BatchGetSecretValueError>);

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsSmApiError(SecretsManagerApiErrorType);

impl From<SecretsManagerApiErrorType> for AwsSmApiError {
    fn from(value: SecretsManagerApiErrorType) -> Self {
        Self(value)
    }
}

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsPsSdkError(#[from] ParameterStoreSdkError<GetParametersError>);

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct IoError(#[from] io::Error);

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct SerdeError(#[from] serde_json::Error);

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct TomlDeError(#[from] toml::de::Error);

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct TomlSerError(#[from] toml::ser::Error);
