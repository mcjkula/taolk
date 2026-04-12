use samp::{BlockNumber, ExtIndex};
use taolk::types::{BlockRef, Pubkey};

#[test]
fn pubkey_equality() {
    let a = Pubkey::from_bytes([0xAA; 32]);
    let b = Pubkey::from_bytes([0xAA; 32]);
    assert_eq!(a, b);
}

#[test]
fn pubkey_inequality() {
    let a = Pubkey::from_bytes([0xAA; 32]);
    let b = Pubkey::from_bytes([0xBB; 32]);
    assert_ne!(a, b);
}

#[test]
fn pubkey_zero() {
    assert_eq!(Pubkey::ZERO.as_bytes(), &[0u8; 32]);
}

#[test]
fn pubkey_as_bytes() {
    let pk = Pubkey::from_bytes([0xCC; 32]);
    assert_eq!(pk.as_bytes(), &[0xCC; 32]);
}

#[test]
fn pubkey_from_bytes_round_trip() {
    let pk = Pubkey::from_bytes([1u8; 32]);
    assert_eq!(pk.as_bytes(), &[1u8; 32]);
}

#[test]
fn pubkey_into_bytes() {
    let pk = Pubkey::from_bytes([0xDD; 32]);
    let bytes: [u8; 32] = pk.into_bytes();
    assert_eq!(bytes, [0xDD; 32]);
}

#[test]
fn blockref_zero() {
    assert_eq!(BlockRef::ZERO.block().get(), 0);
    assert_eq!(BlockRef::ZERO.index().get(), 0);
}

#[test]
fn blockref_is_zero() {
    assert!(BlockRef::ZERO.is_zero());
    assert!(!BlockRef::new(BlockNumber::new(1), ExtIndex::new(0)).is_zero());
}

#[test]
fn blockref_ordering() {
    let a = BlockRef::new(BlockNumber::new(0), ExtIndex::new(5));
    let b = BlockRef::new(BlockNumber::new(1), ExtIndex::new(0));
    assert!(b > a);

    let c = BlockRef::new(BlockNumber::new(1), ExtIndex::new(0));
    let d = BlockRef::new(BlockNumber::new(1), ExtIndex::new(3));
    assert!(d > c);
}

#[test]
fn blockref_hash_consistent() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let a = BlockRef::from_parts(42, 7);
    let b = BlockRef::from_parts(42, 7);

    let mut h1 = DefaultHasher::new();
    a.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    b.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}

// --- Timestamp ---

#[test]
fn timestamp_from_unix_secs_round_trip() {
    let ts = taolk::types::Timestamp::from_unix_secs(1234567890);
    assert_eq!(ts.as_unix_secs(), 1234567890);
}

#[test]
fn timestamp_zero_constant() {
    assert_eq!(taolk::types::Timestamp::ZERO.as_unix_secs(), 0);
}

#[test]
fn timestamp_debug_format() {
    let ts = taolk::types::Timestamp::from_unix_secs(42);
    assert_eq!(format!("{ts:?}"), "Timestamp(42s)");
}

#[test]
fn timestamp_ordering() {
    let a = taolk::types::Timestamp::from_unix_secs(10);
    let b = taolk::types::Timestamp::from_unix_secs(20);
    assert!(b > a);
}

// --- MessageBody ---

use taolk::types::MessageBody;

#[test]
fn message_body_parse_valid() {
    let body = MessageBody::parse("hello").unwrap();
    assert_eq!(body.as_str(), "hello");
    assert_eq!(body.len(), 5);
    assert!(!body.is_empty());
}

#[test]
fn message_body_parse_empty() {
    let body = MessageBody::parse("").unwrap();
    assert!(body.is_empty());
}

#[test]
fn message_body_parse_max_boundary() {
    let s = "x".repeat(taolk::types::MESSAGE_BODY_MAX_BYTES);
    assert!(MessageBody::parse(s).is_ok());
}

#[test]
fn message_body_parse_over_max() {
    let s = "x".repeat(taolk::types::MESSAGE_BODY_MAX_BYTES + 1);
    assert!(MessageBody::parse(s).is_err());
}

#[test]
fn message_body_into_string() {
    let body = MessageBody::parse("test").unwrap();
    assert_eq!(body.into_string(), "test");
}

// --- ChainName ---

use taolk::types::ChainName;

#[test]
fn chain_name_parse_valid() {
    let cn = ChainName::parse("Bittensor").unwrap();
    assert_eq!(cn.as_str(), "Bittensor");
}

#[test]
fn chain_name_parse_empty() {
    assert!(ChainName::parse("").is_err());
}

#[test]
fn chain_name_parse_over_max() {
    let s = "a".repeat(taolk::types::CHAIN_NAME_MAX_BYTES + 1);
    assert!(ChainName::parse(s).is_err());
}

#[test]
fn chain_name_parse_max_boundary() {
    let s = "a".repeat(taolk::types::CHAIN_NAME_MAX_BYTES);
    assert!(ChainName::parse(s).is_ok());
}

// --- NodeUrl ---

use taolk::types::NodeUrl;

#[test]
fn node_url_parse_valid_wss() {
    let url = NodeUrl::parse("wss://example.com").unwrap();
    assert_eq!(url.as_str(), "wss://example.com");
}

#[test]
fn node_url_parse_valid_ws() {
    let url = NodeUrl::parse("ws://localhost:9944").unwrap();
    assert_eq!(url.as_str(), "ws://localhost:9944");
}

#[test]
fn node_url_parse_invalid() {
    assert!(NodeUrl::parse("not-a-url").is_err());
}

#[test]
fn node_url_parse_http_rejected() {
    assert!(NodeUrl::parse("http://example.com").is_err());
}

#[test]
fn node_url_into_string() {
    let url = NodeUrl::parse("wss://example.com").unwrap();
    assert_eq!(url.into_string(), "wss://example.com");
}

// --- WalletName ---

use taolk::types::WalletName;

#[test]
fn wallet_name_parse_valid() {
    let wn = WalletName::parse("my-wallet_01").unwrap();
    assert_eq!(wn.as_str(), "my-wallet_01");
}

#[test]
fn wallet_name_parse_empty() {
    assert!(WalletName::parse("").is_err());
}

#[test]
fn wallet_name_parse_special_chars() {
    assert!(WalletName::parse("no spaces").is_err());
    assert!(WalletName::parse("no.dots").is_err());
}

#[test]
fn wallet_name_into_string() {
    let wn = WalletName::parse("test").unwrap();
    assert_eq!(wn.into_string(), "test");
}

// --- MirrorUrl ---

use taolk::types::MirrorUrl;

#[test]
fn mirror_url_parse_valid() {
    let mu = MirrorUrl::parse("https://mirror.example.com").unwrap();
    assert_eq!(mu.as_str(), "https://mirror.example.com");
}

#[test]
fn mirror_url_parse_strips_trailing_slash() {
    let mu = MirrorUrl::parse("https://mirror.example.com/").unwrap();
    assert_eq!(mu.as_str(), "https://mirror.example.com");
}

#[test]
fn mirror_url_parse_ws_rejected() {
    assert!(MirrorUrl::parse("wss://example.com").is_err());
}

// --- ChainId ---

use taolk::types::ChainId;

#[test]
fn chain_id_from_bytes_round_trip() {
    let id = ChainId::from_bytes([0xDE, 0xAD, 0xBE, 0xEF]);
    assert_eq!(id.as_bytes(), &[0xDE, 0xAD, 0xBE, 0xEF]);
}
