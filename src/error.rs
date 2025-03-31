use aws_sdk_secretsmanager::{
    error::SdkError, operation::batch_get_secret_value::BatchGetSecretValueError,
    types::ApiErrorType,
};

use thiserror::Error;

use crate::parser::Rule;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum Error {
    #[error("Unable to parse input:\n{0}")]
    ParsingError(#[from] pest::error::Error<Rule>),

    #[error("AWS SDK error: {0}")]
    AwsSdkError(#[from] AwsSdkError),

    #[error("AWS API error: {0}")]
    AwsApiError(#[from] AwsApiError),
}

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsSdkError(#[from] SdkError<BatchGetSecretValueError>);

impl PartialEq for AwsSdkError {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for AwsSdkError {}

#[derive(Error, Debug)]
#[error("{0:#?}")]
pub struct AwsApiError(ApiErrorType);

impl PartialEq for AwsApiError {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for AwsApiError {}

impl From<ApiErrorType> for AwsApiError {
    fn from(value: ApiErrorType) -> Self {
        Self(value)
    }
}
