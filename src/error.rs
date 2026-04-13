use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WalletError {
    #[error("Wrong password")]
    WrongPassword,
    #[error("Wallet file is corrupt")]
    CorruptFile,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AddressError {
    #[error("address too short")]
    TooShort,
    #[error("invalid checksum")]
    BadChecksum,
    #[error("invalid base58 character")]
    InvalidBase58,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ChainError {
    #[error("connect: {0}")]
    Connect(String),
    #[error("send: {0}")]
    Send(String),
    #[error("ws closed")]
    WsClosed,
    #[error("ws: {0}")]
    Ws(String),
    #[error("RPC parse: {0}")]
    Parse(String),
    #[error("RPC error: {0}")]
    Rpc(String),
    #[error("transaction failed: {0}")]
    TxFailed(String),
    #[error("submission timed out after 60s")]
    Timeout,
    #[error("hex decode: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("expected 32 bytes")]
    BadLength,
    #[error("missing field {0}")]
    MissingField(&'static str),
    #[error("unexpected response shape")]
    BadShape,
    #[error("metadata: {0}")]
    Metadata(#[from] samp::metadata::Error),
    #[error("extrinsic: {0}")]
    ExtrinsicBuild(#[from] samp::extrinsic::Error),
    #[error("message too long: {len} bytes (max u32::MAX)")]
    MessageTooLong { len: usize },
    #[error("spec/tx version overflow: {0}")]
    SpecVersionOverflow(u64),
    #[error(
        "mirror chain mismatch: serves '{chain}' (SS58 prefix {got}), expected prefix {expected}"
    )]
    MirrorChainMismatch {
        chain: String,
        got: u16,
        expected: u16,
    },
    #[error("http: {0}")]
    Http(String),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml serialize: {0}")]
    TomlSer(#[from] toml::ser::Error),
    #[error("unknown key: {0}")]
    UnknownKey(String),
    #[error("expected {expected}, got '{got}'")]
    InvalidValue { expected: String, got: String },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SdkError {
    #[error("encryption failed: {0}")]
    Encryption(String),
    #[error("decryption failed: {0}")]
    Decryption(String),
    #[error(transparent)]
    Address(#[from] AddressError),
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error(transparent)]
    Wallet(#[from] WalletError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Metadata(#[from] samp::metadata::Error),
    #[error("database: {0}")]
    Database(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Other(String),
}

impl From<rusqlite::Error> for SdkError {
    fn from(e: rusqlite::Error) -> Self {
        SdkError::Database(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SdkError>;
