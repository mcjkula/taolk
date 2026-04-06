use thiserror::Error;

#[derive(Error, Debug)]
pub enum SdkError {
    #[error("encryption failed: {0}")]
    Encryption(String),
    #[error("decryption failed: {0}")]
    Decryption(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("chain error: {0}")]
    Chain(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(String),
    #[error("wallet error: {0}")]
    Wallet(String),
    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, SdkError>;
