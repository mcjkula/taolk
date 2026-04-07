mod common;

use common::{ALICE_SEED, signing_from_seed, test_chain_info};
use taolk::extrinsic;

#[test]
fn build_remark_stable_length() {
    let sk = signing_from_seed(&ALICE_SEED);
    let ci = test_chain_info();
    let remark = b"hello";

    let a = extrinsic::build_remark_extrinsic(remark, &sk, 0, &ci).unwrap();
    let b = extrinsic::build_remark_extrinsic(remark, &sk, 0, &ci).unwrap();

    assert_eq!(a.len(), b.len());
}

#[test]
fn build_remark_different_nonces() {
    let sk = signing_from_seed(&ALICE_SEED);
    let ci = test_chain_info();
    let remark = b"hello";

    let a = extrinsic::build_remark_extrinsic(remark, &sk, 0, &ci).unwrap();
    let b = extrinsic::build_remark_extrinsic(remark, &sk, 1, &ci).unwrap();

    assert_ne!(a, b);
}

#[test]
fn build_remark_contains_payload() {
    let sk = signing_from_seed(&ALICE_SEED);
    let ci = test_chain_info();
    let remark = b"unique-payload-marker";

    let ext = extrinsic::build_remark_extrinsic(remark, &sk, 0, &ci).unwrap();

    let found = ext.windows(remark.len()).any(|w| w == remark);
    assert!(
        found,
        "extrinsic should contain the remark payload as a byte substring"
    );
}

#[test]
fn build_remark_starts_with_length_prefix() {
    let sk = signing_from_seed(&ALICE_SEED);
    let ci = test_chain_info();
    let remark = b"test";

    let ext = extrinsic::build_remark_extrinsic(remark, &sk, 0, &ci).unwrap();

    // Decode compact length from first byte(s)
    let (prefix_len, encoded_len) = decode_compact(&ext);
    // The encoded length should equal the remaining bytes after the prefix
    assert_eq!(
        encoded_len,
        ext.len() - prefix_len,
        "compact length prefix should encode the length of the remaining extrinsic payload"
    );
}

#[test]
fn build_remark_system_pallet() {
    let sk = signing_from_seed(&ALICE_SEED);
    let ci = test_chain_info();
    let remark = b"pallet-test";

    let ext = extrinsic::build_remark_extrinsic(remark, &sk, 0, &ci).unwrap();

    // Layout after compact length prefix:
    // 0x84 (signed flag) | 0x00 (addr type) | account_id(32) | 0x01 (sig type) | sig(64)
    // | era(1) | compact_nonce | tip(1) | metadata_hash_disabled(1) | call_data...
    // call_data starts with pallet_index(1) | call_index(1) | compact_remark_len | remark
    let (prefix_len, _) = decode_compact(&ext);
    let payload = &ext[prefix_len..];

    // Find the call data after the fixed-size header:
    // 0x84(1) + addr_type(1) + account_id(32) + sig_type(1) + sig(64) = 99 bytes
    // then era(1) + compact_nonce + tip(1) + metadata_hash_disabled(1) + call_data...
    assert_eq!(payload[0], 0x84, "first byte should be EXT_VERSION_SIGNED");
    let era_offset = 99; // 1 + 1 + 32 + 1 + 64
    assert_eq!(payload[era_offset], 0x00, "era should be immortal (0x00)");

    // After era: compact nonce (for nonce=0, compact is 0x00 = 1 byte) + tip(1) + metadata_hash(1)
    let call_offset = era_offset + 1 + 1 + 1 + 1; // era + nonce(compact 0) + tip + metadata_hash
    assert_eq!(
        payload[call_offset], 0x00,
        "pallet index should be 0x00 (System)"
    );
    assert_eq!(
        payload[call_offset + 1],
        0x07,
        "call index should be 0x07 (remark_with_event)"
    );
}

#[test]
fn build_remark_immortal_era() {
    let sk = signing_from_seed(&ALICE_SEED);
    let ci = test_chain_info();
    let remark = b"era-test";

    let ext = extrinsic::build_remark_extrinsic(remark, &sk, 0, &ci).unwrap();

    let (prefix_len, _) = decode_compact(&ext);
    let payload = &ext[prefix_len..];

    // Era is at offset 99 (after 0x84(1) + addr_type(1) + account_id(32) + sig_type(1) + sig(64))
    let era_offset = 1 + 1 + 32 + 1 + 64;
    assert_eq!(
        payload[era_offset], 0x00,
        "era byte should be 0x00 (immortal)"
    );
}

fn decode_compact(data: &[u8]) -> (usize, usize) {
    match data[0] & 0b11 {
        0b00 => (1, (data[0] >> 2) as usize),
        0b01 => {
            let val = u16::from_le_bytes([data[0], data[1]]) >> 2;
            (2, val as usize)
        }
        0b10 => {
            let val = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) >> 2;
            (4, val as usize)
        }
        _ => panic!("big-integer compact not expected in tests"),
    }
}
