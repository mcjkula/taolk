use taolk::types::Pubkey;
use taolk::util;

#[test]
fn ss58_from_pubkey_deterministic() {
    let pk = Pubkey([0xAA; 32]);
    assert_eq!(util::ss58_from_pubkey(&pk), util::ss58_from_pubkey(&pk));
}

#[test]
fn ss58_from_pubkey_starts_with_5() {
    let pk = Pubkey([0xAA; 32]);
    let addr = util::ss58_from_pubkey(&pk);
    assert!(
        addr.starts_with('5'),
        "SS58 address should start with '5', got: {addr}"
    );
}

#[test]
fn ss58_decode_roundtrip() {
    let pk = Pubkey([0xBB; 32]);
    let addr = util::ss58_from_pubkey(&pk);
    let decoded = util::ss58_decode(&addr).unwrap();
    assert_eq!(decoded, pk);
}

#[test]
fn ss58_decode_invalid_base58() {
    assert!(util::ss58_decode("!!invalid!!").is_err());
}

#[test]
fn ss58_decode_wrong_checksum() {
    let pk = Pubkey([0xCC; 32]);
    let mut addr = util::ss58_from_pubkey(&pk);
    let last = addr.pop().unwrap();
    let replacement = if last == 'A' { 'B' } else { 'A' };
    addr.push(replacement);
    assert!(util::ss58_decode(&addr).is_err());
}

#[test]
fn ss58_short_format() {
    let pk = Pubkey([0xDD; 32]);
    let short = util::ss58_short(&pk);
    assert_eq!(short.len(), 13);
    assert!(short.contains("..."));
}

#[test]
fn pubkey_from_ss58_valid() {
    let pk = Pubkey([0xEE; 32]);
    let addr = util::ss58_from_pubkey(&pk);
    assert_eq!(util::pubkey_from_ss58(&addr), Some(pk));
}

#[test]
fn pubkey_from_ss58_invalid() {
    assert_eq!(util::pubkey_from_ss58("garbage"), None);
}

#[test]
fn truncate_short_string() {
    assert_eq!(util::truncate("hello", 10), "hello");
}

#[test]
fn truncate_long_string() {
    let result = util::truncate("hello world, this is a long string", 16);
    assert!(result.contains("..."));
    assert!(result.len() <= 16);
}

#[test]
fn format_balance_basic() {
    let result = util::format_balance(1_000_000_000, 9, "TAO");
    assert!(
        result.contains("1"),
        "Expected '1' in balance, got: {result}"
    );
    assert!(
        result.contains("\u{03C4}"),
        "Expected tau symbol in balance, got: {result}"
    );
}

#[test]
fn format_balance_zero() {
    let result = util::format_balance(0, 9, "TAO");
    assert!(
        result.contains("0"),
        "Expected '0' in balance, got: {result}"
    );
    assert!(
        result.contains("\u{03C4}"),
        "Expected tau symbol, got: {result}"
    );
}

#[test]
fn format_fee_basic() {
    let result = util::format_fee(500_000, 9, "TAO");
    assert!(result.contains("500,000"));
    assert!(result.contains("RAO"));
}

#[test]
fn format_number_with_commas() {
    assert_eq!(util::format_number(1234567), "1,234,567");
}
