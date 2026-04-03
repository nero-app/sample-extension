use std::str;

use thiserror::Error;

use crate::wasi::http::types::{ErrorCode, HeaderError};

#[derive(Error, Debug)]
pub enum Error {
    #[cfg(feature = "json")]
    #[error("JSON serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),

    #[error("Invalid UTF-8 in body: {0}")]
    InvalidUtf8InBody(str::Utf8Error),

    #[error("Header error: {0}")]
    Header(#[from] HeaderError),

    #[error("HTTP error")]
    Http(#[from] ErrorCode),
}

impl From<Error> for ErrorCode {
    fn from(error: Error) -> Self {
        match error {
            Error::Http(error_code) => error_code,
            other => ErrorCode::InternalError(Some(other.to_string())),
        }
    }
}
