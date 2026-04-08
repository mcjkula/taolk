pub use samp::{BlockRef, Pubkey};
use zeroize::Zeroizing;

use crate::error::SdkError;

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
