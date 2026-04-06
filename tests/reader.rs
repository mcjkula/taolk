use schnorrkel::keys::{ExpansionMode, MiniSecretKey};
use std::sync::mpsc;
use taolk::event::Event;
use taolk::extrinsic::{self, ChainInfo};
use taolk::metadata::AccountInfoLayout;
use taolk::reader::{self, ReadContext};
use taolk::types::Pubkey;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn keypair(seed: &[u8; 32]) -> schnorrkel::Keypair {
    MiniSecretKey::from_bytes(seed)
        .unwrap()
        .expand_to_keypair(ExpansionMode::Ed25519)
}

fn ci() -> ChainInfo {
    ChainInfo {
        genesis_hash: [0; 32],
        spec_version: 1,
        tx_version: 1,
        account_info_layout: AccountInfoLayout {
            free_offset: 16,
            free_width: 8,
        },
    }
}

fn ext_to_hex(ext_bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(ext_bytes))
}

/// Build a ReadContext targeting the given seed/pubkey, returning the context and receiver.
fn make_ctx<'a>(
    seed: &'a [u8; 32],
    pubkey: &'a Pubkey,
    tx: &'a mpsc::Sender<Event>,
) -> ReadContext<'a> {
    ReadContext {
        my_pubkey: pubkey,
        seed,
        tx,
    }
}

// ---------------------------------------------------------------------------
// 1. read_extrinsic emits Event::NewMessage for a valid SAMP public remark
// ---------------------------------------------------------------------------

#[test]
fn read_extrinsic_emits_event_for_samp_remark() {
    let alice_seed = [0xAA; 32];
    let alice_kp = keypair(&alice_seed);
    let alice_pubkey = Pubkey(alice_kp.public.to_bytes());

    let bob_seed = [0xBB; 32];
    let bob_pubkey = Pubkey(keypair(&bob_seed).public.to_bytes());

    // Alice sends a public message to Bob
    let remark = samp::encode_public(&bob_pubkey.0, b"hello bob");
    let ext = extrinsic::build_remark_extrinsic(&remark, &alice_kp, 0, &ci());
    let hex = ext_to_hex(&ext);

    // Bob reads the extrinsic
    let (tx, rx) = mpsc::channel();
    let ctx = make_ctx(&bob_seed, &bob_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 100, 1, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::NewMessage {
            sender,
            content_type,
            recipient,
            decrypted_body,
            block_number,
            ext_index,
            timestamp,
            ..
        }) => {
            assert_eq!(sender, alice_pubkey);
            assert_eq!(content_type, 0x10);
            assert_eq!(recipient, bob_pubkey);
            assert_eq!(decrypted_body, Some("hello bob".to_string()));
            assert_eq!(block_number, 100);
            assert_eq!(ext_index, 1);
            assert_eq!(timestamp, 1_700_000_000);
        }
        other => panic!("expected NewMessage, got {:?}", event_debug(&other)),
    }
}

// ---------------------------------------------------------------------------
// 2. read_extrinsic ignores non-SAMP remark
// ---------------------------------------------------------------------------

#[test]
fn read_extrinsic_ignores_non_samp_remark() {
    let alice_seed = [0xAA; 32];
    let alice_kp = keypair(&alice_seed);
    let alice_pubkey = Pubkey(alice_kp.public.to_bytes());

    // Build an extrinsic with a non-SAMP remark (no 0x1X content type prefix)
    let remark = b"not a samp message";
    let ext = extrinsic::build_remark_extrinsic(remark, &alice_kp, 0, &ci());
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let ctx = make_ctx(&alice_seed, &alice_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 100, 0, 0);

    assert!(
        rx.try_recv().is_err(),
        "non-SAMP remark should not emit any events"
    );
}

// ---------------------------------------------------------------------------
// 3. extract_block_timestamp from a real timestamp inherent
// ---------------------------------------------------------------------------

#[test]
fn extract_block_timestamp_from_inherent() {
    // Build a minimal unsigned Substrate timestamp inherent:
    // SCALE compact length prefix || version(1, unsigned = 0x04) || pallet(0x03) || call(0x00) || compact_timestamp
    //
    // Timestamp pallet = 0x03, set call = 0x00
    // We encode timestamp 1_700_000_000_000 ms using SCALE compact u64.
    let ts_ms: u64 = 1_700_000_000_000;

    // SCALE compact encoding of u64 > 2^30 uses big-integer mode (mode 0b11):
    // first byte = (bytes_following - 4) << 2 | 0b11
    // For u64: bytes_following = 8, so first byte = (8-4)<<2 | 3 = 0x13
    let ts_le = ts_ms.to_le_bytes();
    let mut compact_ts = vec![0x13u8]; // (4 << 2) | 0b11 = 0x13
    compact_ts.extend_from_slice(&ts_le);

    let mut payload = Vec::new();
    payload.push(0x04); // extrinsic version byte: unsigned (no SIGNED_BIT)
    payload.push(0x03); // Timestamp pallet
    payload.push(0x00); // set call
    payload.extend_from_slice(&compact_ts);

    // Wrap with SCALE compact length prefix
    let mut full = Vec::new();
    let len = payload.len() as u8;
    full.push(len << 2); // single-byte compact for small lengths (mode 0b00)
    full.extend_from_slice(&payload);

    let hex = ext_to_hex(&full);
    let extrinsics = vec![serde_json::Value::String(hex)];

    let result = reader::extract_block_timestamp(&extrinsics);
    assert_eq!(result, ts_ms);
}

// ---------------------------------------------------------------------------
// 4. extract_block_timestamp returns 0 for empty extrinsics array
// ---------------------------------------------------------------------------

#[test]
fn extract_block_timestamp_empty() {
    let extrinsics: Vec<serde_json::Value> = vec![];
    assert_eq!(reader::extract_block_timestamp(&extrinsics), 0);
}

// ---------------------------------------------------------------------------
// 5. read_extrinsic decrypts encrypted message for correct recipient
// ---------------------------------------------------------------------------

#[test]
fn read_extrinsic_decrypts_for_recipient() {
    let alice_seed = [0xAA; 32];
    let alice_kp = keypair(&alice_seed);
    let alice_pubkey = Pubkey(alice_kp.public.to_bytes());

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&bob_seed);
    // Bob's sr25519 public key (for Substrate account) comes from the keypair
    let bob_kp = keypair(&bob_seed);
    let bob_pubkey = Pubkey(bob_kp.public.to_bytes());

    let plaintext = b"secret for bob";
    let nonce: [u8; 12] = [0x01; 12];

    // Compute view tag and encrypt
    let recipient_ristretto = curve25519_dalek::ristretto::CompressedRistretto(bob_ristretto_pub);
    let view_tag = samp::compute_view_tag(&alice_seed, &recipient_ristretto, &nonce).unwrap();
    let encrypted_content =
        samp::encrypt(plaintext, &recipient_ristretto, &nonce, &alice_seed).unwrap();

    // Build the SAMP remark wire format for encrypted (0x11)
    let remark = samp::encode_encrypted(0x11, view_tag, &nonce, &encrypted_content);

    let ext = extrinsic::build_remark_extrinsic(&remark, &alice_kp, 0, &ci());
    let hex = ext_to_hex(&ext);

    // Bob reads
    let (tx, rx) = mpsc::channel();
    let ctx = make_ctx(&bob_seed, &bob_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 200, 3, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::NewMessage {
            sender,
            content_type,
            decrypted_body,
            block_number,
            ext_index,
            timestamp,
            ..
        }) => {
            assert_eq!(sender, alice_pubkey);
            assert_eq!(content_type, 0x11);
            assert_eq!(decrypted_body, Some("secret for bob".to_string()));
            assert_eq!(block_number, 200);
            assert_eq!(ext_index, 3);
            assert_eq!(timestamp, 1_700_000_000);
        }
        other => panic!("expected NewMessage, got {:?}", event_debug(&other)),
    }
}

// ---------------------------------------------------------------------------
// 6. read_extrinsic skips encrypted message for wrong recipient
// ---------------------------------------------------------------------------

#[test]
fn read_extrinsic_skips_message_for_wrong_recipient() {
    let alice_seed = [0xAA; 32];
    let alice_kp = keypair(&alice_seed);

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&bob_seed);

    let charlie_seed = [0xCC; 32];
    let charlie_kp = keypair(&charlie_seed);
    let charlie_pubkey = Pubkey(charlie_kp.public.to_bytes());

    let plaintext = b"for bob only";
    let nonce: [u8; 12] = [0x02; 12];

    // Alice encrypts for Bob
    let recipient_ristretto = curve25519_dalek::ristretto::CompressedRistretto(bob_ristretto_pub);
    let view_tag = samp::compute_view_tag(&alice_seed, &recipient_ristretto, &nonce).unwrap();
    let encrypted_content =
        samp::encrypt(plaintext, &recipient_ristretto, &nonce, &alice_seed).unwrap();
    let remark = samp::encode_encrypted(0x11, view_tag, &nonce, &encrypted_content);

    let ext = extrinsic::build_remark_extrinsic(&remark, &alice_kp, 0, &ci());
    let hex = ext_to_hex(&ext);

    // Charlie tries to read (should not match -- view tag mismatch filters it)
    let (tx, rx) = mpsc::channel();
    let ctx = make_ctx(&charlie_seed, &charlie_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 200, 3, 0);

    assert!(
        rx.try_recv().is_err(),
        "encrypted message for Bob should not emit event when read by Charlie"
    );
}

// ---------------------------------------------------------------------------
// Debug helper for test failures (Event doesn't derive Debug)
// ---------------------------------------------------------------------------

fn event_debug(result: &Result<Event, mpsc::TryRecvError>) -> String {
    match result {
        Ok(Event::NewMessage {
            sender,
            content_type,
            ..
        }) => {
            format!("NewMessage(sender={:?}, ct=0x{:02x})", sender, content_type)
        }
        Ok(Event::NewChannelMessage { .. }) => "NewChannelMessage".to_string(),
        Ok(Event::ChannelDiscovered { .. }) => "ChannelDiscovered".to_string(),
        Ok(Event::GroupDiscovered { .. }) => "GroupDiscovered".to_string(),
        Ok(Event::NewGroupMessage { .. }) => "NewGroupMessage".to_string(),
        Ok(Event::BlockUpdate(n)) => format!("BlockUpdate({n})"),
        Ok(Event::MessageSent) => "MessageSent".to_string(),
        Ok(Event::Status(s)) => format!("Status({s})"),
        Ok(Event::Error(e)) => format!("Error({e})"),
        Ok(_) => "Other event".to_string(),
        Err(e) => format!("Err({e:?})"),
    }
}
