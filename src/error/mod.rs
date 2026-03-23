pub mod error_code;

use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Error {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("serde error: {0}")]
    SerdeError(#[from] serde_json::Error),

    #[error("config error: {0}")]
    Config(#[from] toolcraft_config::error::Error),

    #[error("request error: {0}")]
    Request(#[from] toolcraft_request::error::Error),

    #[error("jwt error: {0}")]
    Jwt(#[from] toolcraft_jwt::error::Error),

    #[error("{0}")]
    Message(String),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;
