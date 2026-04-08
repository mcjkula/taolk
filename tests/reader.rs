mod common;

use common::{build_remark_ext, signing_from_seed as signing};
use std::sync::mpsc;
use taolk::event::Event;
use taolk::reader::{self, ReadContext};
use taolk::secret::DecryptionKeys;
use taolk::types::Pubkey;

fn ext_to_hex(ext_bytes: &[u8]) -> String {
    format!("0x{}", hex::encode(ext_bytes))
}

fn make_keys(seed: &[u8; 32]) -> DecryptionKeys {
    let view_scalar = samp::sr25519_signing_scalar(seed).to_bytes();
    DecryptionKeys::new(view_scalar, Some(*seed))
}

fn make_ctx<'a>(
    keys: &'a DecryptionKeys,
    pubkey: &'a Pubkey,
    tx: &'a mpsc::Sender<Event>,
) -> ReadContext<'a> {
    ReadContext {
        my_pubkey: pubkey,
        keys,
        tx,
    }
}

#[test]
fn read_extrinsic_emits_event_for_samp_remark() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let bob_seed = [0xBB; 32];
    let bob_pubkey = signing(&bob_seed).public_key();

    let remark = samp::encode_public(&bob_pubkey.0, b"hello bob");
    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
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

#[test]
fn read_extrinsic_ignores_non_samp_remark() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let remark = b"not a samp message";
    let ext = build_remark_ext(remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&alice_seed);
    let ctx = make_ctx(&keys, &alice_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 100, 0, 0);

    assert!(
        rx.try_recv().is_err(),
        "non-SAMP remark should not emit any events"
    );
}

#[test]
fn extract_block_timestamp_from_inherent() {
    let ts_ms: u64 = 1_700_000_000_000;

    let ts_le = ts_ms.to_le_bytes();
    let mut compact_ts = vec![0x13u8];
    compact_ts.extend_from_slice(&ts_le);

    let mut payload = Vec::new();
    payload.push(0x04);
    payload.push(0x03);
    payload.push(0x00);
    payload.extend_from_slice(&compact_ts);

    let mut full = Vec::new();
    let len = payload.len() as u8;
    full.push(len << 2);
    full.extend_from_slice(&payload);

    let hex = ext_to_hex(&full);
    let extrinsics = vec![serde_json::Value::String(hex)];

    let result = reader::extract_block_timestamp(&extrinsics);
    assert_eq!(result, ts_ms);
}

#[test]
fn extract_block_timestamp_empty() {
    let extrinsics: Vec<serde_json::Value> = vec![];
    assert_eq!(reader::extract_block_timestamp(&extrinsics), 0);
}

#[test]
fn read_extrinsic_decrypts_for_recipient() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&bob_seed);
    let bob_sk = signing(&bob_seed);
    let bob_pubkey = bob_sk.public_key();

    let plaintext = b"secret for bob";
    let nonce: [u8; 12] = [0x01; 12];

    let recipient_ristretto = curve25519_dalek::ristretto::CompressedRistretto(bob_ristretto_pub);
    let view_tag = samp::compute_view_tag(&alice_seed, &recipient_ristretto, &nonce).unwrap();
    let encrypted_content =
        samp::encrypt(plaintext, &recipient_ristretto, &nonce, &alice_seed).unwrap();

    let remark = samp::encode_encrypted(
        samp::CONTENT_TYPE_ENCRYPTED,
        view_tag,
        &nonce,
        &encrypted_content,
    );

    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
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

#[test]
fn read_extrinsic_skips_message_for_wrong_recipient() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&bob_seed);

    let charlie_seed = [0xCC; 32];
    let charlie_sk = signing(&charlie_seed);
    let charlie_pubkey = charlie_sk.public_key();

    let plaintext = b"for bob only";
    let nonce: [u8; 12] = [0x02; 12];

    let recipient_ristretto = curve25519_dalek::ristretto::CompressedRistretto(bob_ristretto_pub);
    let view_tag = samp::compute_view_tag(&alice_seed, &recipient_ristretto, &nonce).unwrap();
    let encrypted_content =
        samp::encrypt(plaintext, &recipient_ristretto, &nonce, &alice_seed).unwrap();
    let remark = samp::encode_encrypted(
        samp::CONTENT_TYPE_ENCRYPTED,
        view_tag,
        &nonce,
        &encrypted_content,
    );

    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&charlie_seed);
    let ctx = make_ctx(&keys, &charlie_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 200, 3, 0);

    assert!(
        rx.try_recv().is_err(),
        "encrypted message for Bob should not emit event when read by Charlie"
    );
}

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
