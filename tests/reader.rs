mod common;

use common::{build_remark_ext, signing_from_seed as signing};
use std::sync::mpsc;
use taolk::event::Event;
use taolk::reader::{self, ReadContext};
use taolk::secret::DecryptionKeys;
use taolk::types::Pubkey;

fn ext_to_hex(ext_bytes: &samp::ExtrinsicBytes) -> String {
    format!("0x{}", hex::encode(ext_bytes.as_bytes()))
}

fn make_keys(seed: &[u8; 32]) -> DecryptionKeys {
    let view_scalar = *samp::sr25519_signing_scalar(&samp::Seed::from_bytes(*seed)).expose_secret();
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

    let remark = samp::encode_public(&bob_pubkey, "hello bob");
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
            assert_eq!(
                timestamp,
                taolk::types::Timestamp::from_unix_secs(1_700_000_000)
            );
        }
        other => panic!("expected NewMessage, got {:?}", event_debug(&other)),
    }
}

#[test]
fn read_extrinsic_ignores_non_samp_remark() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let remark = samp::RemarkBytes::from_bytes(b"not a samp message".to_vec());
    let ext = build_remark_ext(&remark, &alice_sk, 0);
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

    let hex = format!("0x{}", hex::encode(&full));
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
    let bob_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(bob_seed));
    let bob_sk = signing(&bob_seed);
    let bob_pubkey = bob_sk.public_key();

    let plaintext = samp::Plaintext::from_bytes(b"secret for bob".to_vec());
    let nonce = samp::Nonce::from_bytes([0x01; 12]);
    let alice_samp_seed = samp::Seed::from_bytes(alice_seed);

    let view_tag = samp::compute_view_tag(&alice_samp_seed, &bob_ristretto_pub, &nonce).unwrap();
    let encrypted_content =
        samp::encrypt(&plaintext, &bob_ristretto_pub, &nonce, &alice_samp_seed).unwrap();

    let remark = samp::encode_encrypted(
        samp::ContentType::Encrypted,
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
            assert_eq!(
                timestamp,
                taolk::types::Timestamp::from_unix_secs(1_700_000_000)
            );
        }
        other => panic!("expected NewMessage, got {:?}", event_debug(&other)),
    }
}

#[test]
fn read_extrinsic_skips_message_for_wrong_recipient() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(bob_seed));

    let charlie_seed = [0xCC; 32];
    let charlie_sk = signing(&charlie_seed);
    let charlie_pubkey = charlie_sk.public_key();

    let plaintext = samp::Plaintext::from_bytes(b"for bob only".to_vec());
    let nonce = samp::Nonce::from_bytes([0x02; 12]);
    let alice_samp_seed = samp::Seed::from_bytes(alice_seed);

    let view_tag = samp::compute_view_tag(&alice_samp_seed, &bob_ristretto_pub, &nonce).unwrap();
    let encrypted_content =
        samp::encrypt(&plaintext, &bob_ristretto_pub, &nonce, &alice_samp_seed).unwrap();
    let remark = samp::encode_encrypted(
        samp::ContentType::Encrypted,
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

#[test]
fn process_remark_channel_create() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let name = samp::ChannelName::parse("general").unwrap();
    let desc = samp::ChannelDescription::parse("main channel").unwrap();
    let remark = samp::encode_channel_create(&name, &desc);
    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&alice_seed);
    let ctx = make_ctx(&keys, &alice_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 500, 0, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::ChannelDiscovered {
            name,
            description,
            channel_ref,
            ..
        }) => {
            assert_eq!(name, "general");
            assert_eq!(description, "main channel");
            assert_eq!(channel_ref, taolk::types::BlockRef::from_parts(500, 0));
        }
        other => panic!("expected ChannelDiscovered, got {:?}", event_debug(&other)),
    }
}

#[test]
fn process_remark_channel_message() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let channel_ref = taolk::types::BlockRef::from_parts(300, 1);
    let remark = samp::encode_channel_msg(
        channel_ref,
        taolk::types::BlockRef::ZERO,
        taolk::types::BlockRef::ZERO,
        "hello channel",
    );
    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&alice_seed);
    let ctx = make_ctx(&keys, &alice_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 600, 2, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::NewChannelMessage {
            sender,
            channel_ref: wire_ref,
            body,
            block_number,
            ext_index,
            ..
        }) => {
            assert_eq!(sender, alice_pubkey);
            assert_eq!(wire_ref, channel_ref);
            assert_eq!(body, "hello channel");
            assert_eq!(block_number, 600);
            assert_eq!(ext_index, 2);
        }
        other => panic!("expected NewChannelMessage, got {:?}", event_debug(&other)),
    }
}

#[test]
fn process_remark_locked_outbound() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(bob_seed));

    let plaintext = samp::Plaintext::from_bytes(b"outbound secret".to_vec());
    let nonce = samp::Nonce::from_bytes([0x05; 12]);
    let alice_samp_seed = samp::Seed::from_bytes(alice_seed);

    let view_tag = samp::compute_view_tag(&alice_samp_seed, &bob_ristretto_pub, &nonce).unwrap();
    let encrypted =
        samp::encrypt(&plaintext, &bob_ristretto_pub, &nonce, &alice_samp_seed).unwrap();
    let remark = samp::encode_encrypted(samp::ContentType::Encrypted, view_tag, &nonce, &encrypted);

    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let view_scalar =
        *samp::sr25519_signing_scalar(&samp::Seed::from_bytes(alice_seed)).expose_secret();
    let keys = DecryptionKeys::new(view_scalar, None);
    let ctx = make_ctx(&keys, &alice_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 700, 1, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::LockedOutbound {
            sender,
            block_number,
            ext_index,
            ..
        }) => {
            assert_eq!(sender, alice_pubkey);
            assert_eq!(block_number, 700);
            assert_eq!(ext_index, 1);
        }
        other => panic!("expected LockedOutbound, got {:?}", event_debug(&other)),
    }
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
