pub use samp::{BlockRef, Pubkey};
use zeroize::Zeroizing;

use crate::error::SdkError;

pub const MESSAGE_BODY_MAX_BYTES: usize = 4096;

#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct MessageBody(String);

impl MessageBody {
    pub fn parse(s: impl Into<String>) -> Result<Self, SdkError> {
        let s = s.into();
        if s.len() > MESSAGE_BODY_MAX_BYTES {
            return Err(SdkError::Other(format!(
                "message body must be 0..={MESSAGE_BODY_MAX_BYTES} bytes (got {})",
                s.len()
            )));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for MessageBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "MessageBody({} bytes)", self.0.len())
    }
}

pub const CHAIN_NAME_MAX_BYTES: usize = 64;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ChainName(String);

impl ChainName {
    pub fn parse(s: impl Into<String>) -> Result<Self, SdkError> {
        let s = s.into();
        if s.is_empty() || s.len() > CHAIN_NAME_MAX_BYTES {
            return Err(SdkError::Other(format!(
                "chain name must be 1..={CHAIN_NAME_MAX_BYTES} bytes (got {})",
                s.len()
            )));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ChainName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ChainName({:?})", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct WalletName(String);

impl WalletName {
    pub fn parse(s: impl Into<String>) -> Result<Self, SdkError> {
        let s = s.into();
        if s.is_empty() || s.len() > 64 {
            return Err(SdkError::Other(format!(
                "wallet name must be 1..=64 chars (got {})",
                s.len()
            )));
        }
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(SdkError::Other(
                "wallet name may contain only [A-Za-z0-9_-]".into(),
            ));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct NodeUrl(String);

impl NodeUrl {
    pub fn parse(s: impl Into<String>) -> Result<Self, SdkError> {
        let s = s.into();
        if !(s.starts_with("ws://") || s.starts_with("wss://")) {
            return Err(SdkError::Other(format!(
                "node URL must use ws:// or wss:// (got {s})"
            )));
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct MirrorUrl(String);

impl MirrorUrl {
    pub fn parse(s: impl Into<String>) -> Result<Self, SdkError> {
        let s = s.into();
        if !(s.starts_with("http://") || s.starts_with("https://")) {
            return Err(SdkError::Other(format!(
                "mirror URL must use http:// or https:// (got {s})"
            )));
        }
        Ok(Self(s.trim_end_matches('/').to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChainId([u8; 4]);

impl ChainId {
    pub const fn from_bytes(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }

    pub fn from_genesis(genesis: &samp::GenesisHash) -> Self {
        let mut out = [0u8; 4];
        out.copy_from_slice(&genesis.as_bytes()[..4]);
        Self(out)
    }

    pub const fn as_bytes(&self) -> &[u8; 4] {
        &self.0
    }
}

#[derive(Clone)]
pub struct DbKey(Zeroizing<[u8; 32]>);

impl DbKey {
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn expose_secret(&self) -> &[u8; 32] {
        &self.0
    }
}
