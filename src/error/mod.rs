pub mod error_code;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("config error: {0}")]
    ServiceError(#[from] service_utils_rs::error::Error),

    #[error("serde error: {0}")]
    SerdeError(#[from] serde_json::Error),
}

pub type Result<T, E = Error> = core::result::Result<T, E>;
