mod common;

use common::test_db;
use taolk::secret::{Seed, SeedError};
use taolk::util;

#[test]
fn format_number_zero() {
    assert_eq!(util::format_number(0), "0");
}

#[test]
fn format_number_under_thousand() {
    assert_eq!(util::format_number(999), "999");
}

#[test]
fn format_number_thousand_inserts_comma() {
    assert_eq!(util::format_number(1_000), "1,000");
}

#[test]
fn format_number_million() {
    assert_eq!(util::format_number(1_234_567), "1,234,567");
}

#[test]
fn format_number_u64_max_as_u128() {
    let s = util::format_number(u128::from(u64::MAX));
    assert_eq!(s, "18,446,744,073,709,551,615");
}

#[test]
fn format_number_u128_max() {
    let s = util::format_number(u128::MAX);
    assert!(
        s.starts_with(
            "340,282,366,920,938,463,463,374,607,431,768,211,455"
                .chars()
                .next()
                .unwrap()
        )
    );
    assert!(s.ends_with("455"));
    assert!(s.contains(','));
}

#[test]
fn format_balance_zero_plancks() {
    assert_eq!(util::format_balance(0, 9, "TAO"), "0.0 \u{03C4}");
}

#[test]
fn format_balance_one_planck() {
    let s = util::format_balance(1, 9, "TAO");
    assert!(s.contains("0.000000001"));
}

#[test]
fn format_balance_one_whole_token() {
    let s = util::format_balance(1_000_000_000, 9, "TAO");
    assert!(s.contains("1.0"));
}

#[test]
fn format_balance_u128_max_does_not_panic() {
    let s = util::format_balance(u128::MAX, 9, "TAO");
    assert!(!s.is_empty());
}

#[test]
fn format_balance_zero_decimals_rao() {
    let s = util::format_balance(42, 0, "RAO");
    assert!(s.contains("42"));
    assert!(s.contains("RAO"));
}

#[test]
fn format_fee_below_milli_uses_rao_units() {
    let s = util::format_fee(500, 9, "TAO");
    assert!(s.contains("RAO"));
}

#[test]
fn format_fee_above_milli_uses_tao_units() {
    let s = util::format_fee(2_000_000, 9, "TAO");
    assert!(s.contains("\u{03C4}"));
}

#[test]
fn seed_from_hex_exact_64_chars_succeeds() {
    let hex = "ab".repeat(32);
    Seed::from_hex(&hex).expect("64-char hex should parse");
}

#[test]
fn seed_from_hex_63_chars_fails() {
    let hex = "ab".repeat(31) + "a";
    assert!(matches!(
        Seed::from_hex(&hex),
        Err(SeedError::InvalidHex(_)) | Err(SeedError::WrongLength(_))
    ));
}

#[test]
fn seed_from_hex_65_chars_fails() {
    let hex = "ab".repeat(32) + "a";
    assert!(matches!(
        Seed::from_hex(&hex),
        Err(SeedError::InvalidHex(_)) | Err(SeedError::WrongLength(_))
    ));
}

#[test]
fn seed_from_hex_with_0x_prefix_succeeds() {
    let hex = format!("0x{}", "cd".repeat(32));
    Seed::from_hex(&hex).expect("0x-prefixed 64-char hex should parse");
}

#[test]
fn seed_from_hex_empty_fails() {
    assert!(Seed::from_hex("").is_err());
}

#[test]
fn db_inbox_message_with_unix_epoch_timestamp_roundtrips() {
    use chrono::{TimeZone, Utc};
    use taolk::conversation::InboxMessage;

    let db = test_db();
    let msg = InboxMessage {
        peer_ss58: "5Cyqt".into(),
        timestamp: Utc.timestamp_opt(0, 0).single().unwrap(),
        body: "epoch".into(),
        content_type: 0x11,
        is_mine: false,
        block_number: 1,
        ext_index: 0,
    };
    db.insert_inbox(&msg);
    let (inbox, _outbox) = db.load_inbox();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].body, "epoch");
}

#[test]
fn db_inbox_message_with_far_future_timestamp_roundtrips() {
    use chrono::{TimeZone, Utc};
    use taolk::conversation::InboxMessage;

    let db = test_db();
    let ts = Utc
        .with_ymd_and_hms(9999, 12, 31, 23, 59, 59)
        .single()
        .unwrap();
    let msg = InboxMessage {
        peer_ss58: "5Cyqt".into(),
        timestamp: ts,
        body: "future".into(),
        content_type: 0x11,
        is_mine: false,
        block_number: 1,
        ext_index: 0,
    };
    db.insert_inbox(&msg);
    let (inbox, _) = db.load_inbox();
    assert_eq!(inbox.len(), 1);
    assert_eq!(inbox[0].body, "future");
}
