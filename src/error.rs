use aws_sdk_secretsmanager::{
    error::SdkError as SecretsManagerSdkError,
    operation::batch_get_secret_value::BatchGetSecretValueError,
    types::ApiErrorType as SecretsManagerApiErrorType,
};

use aws_sdk_ssm::{
    error::SdkError as ParameterStoreSdkError, operation::get_parameters::GetParametersError,
};

use thiserror::Error;

use crate::parser::Rule;

#[derive(Error, Debug, PartialEq, Eq)]
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
}

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsSmSdkError(#[from] SecretsManagerSdkError<BatchGetSecretValueError>);

impl PartialEq for AwsSmSdkError {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for AwsSmSdkError {}

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsSmApiError(SecretsManagerApiErrorType);

impl PartialEq for AwsSmApiError {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for AwsSmApiError {}

impl From<SecretsManagerApiErrorType> for AwsSmApiError {
    fn from(value: SecretsManagerApiErrorType) -> Self {
        Self(value)
    }
}

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsPsSdkError(#[from] ParameterStoreSdkError<GetParametersError>);

impl PartialEq for AwsPsSdkError {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for AwsPsSdkError {}
