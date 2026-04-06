/// 32-byte SR25519 public key. Not a seed, not a hash.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Pubkey(pub [u8; 32]);

impl Pubkey {
    pub const ZERO: Self = Self([0u8; 32]);
}

impl std::ops::Deref for Pubkey {
    type Target = [u8; 32];
    fn deref(&self) -> &[u8; 32] {
        &self.0
    }
}

impl From<[u8; 32]> for Pubkey {
    fn from(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
}

impl From<Pubkey> for [u8; 32] {
    fn from(pk: Pubkey) -> [u8; 32] {
        pk.0
    }
}

/// Re-export BlockRef from the samp crate for consistent use across the codebase.
pub use samp::BlockRef;
