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

#[test]
fn read_block_emits_block_update_and_message() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let bob_seed = [0xBB; 32];
    let bob_pubkey = signing(&bob_seed).public_key();

    let remark = samp::encode_public(&bob_pubkey, "block hello");
    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let ext_hex = ext_to_hex(&ext);

    let block = serde_json::json!({
        "header": { "number": "0x64" },
        "extrinsics": [ext_hex]
    });

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
    reader::read_block(&block, &ctx);

    let mut got_block_update = false;
    let mut got_message = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            Event::BlockUpdate(n) => {
                assert_eq!(n, 100);
                got_block_update = true;
            }
            Event::NewMessage {
                sender,
                decrypted_body,
                ..
            } => {
                assert_eq!(sender, alice_pubkey);
                assert_eq!(decrypted_body, Some("block hello".to_string()));
                got_message = true;
            }
            _ => {}
        }
    }
    assert!(got_block_update, "should emit BlockUpdate");
    assert!(got_message, "should emit NewMessage");
}

#[test]
fn read_block_no_extrinsics_key() {
    let bob_seed = [0xBB; 32];
    let bob_pubkey = signing(&bob_seed).public_key();

    let block = serde_json::json!({
        "header": { "number": "0x10" }
    });

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
    reader::read_block(&block, &ctx);

    assert!(rx.try_recv().is_err(), "no events for missing extrinsics");
}

#[test]
fn read_block_non_string_extrinsic_skipped() {
    let bob_seed = [0xBB; 32];
    let bob_pubkey = signing(&bob_seed).public_key();

    let block = serde_json::json!({
        "header": { "number": "0x10" },
        "extrinsics": [42, null, true]
    });

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
    reader::read_block(&block, &ctx);

    // Should get only BlockUpdate, no messages
    match rx.try_recv() {
        Ok(Event::BlockUpdate(16)) => {}
        other => panic!("expected BlockUpdate(16), got {:?}", event_debug(&other)),
    }
    assert!(rx.try_recv().is_err());
}

#[test]
fn source_from_extrinsic_invalid_hex() {
    let result = reader::source_from_extrinsic("not-hex!!", 100, 0, 0);
    assert!(result.is_none());
}

#[test]
fn source_from_extrinsic_valid_public() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let bob_pubkey = signing(&[0xBB; 32]).public_key();

    let remark = samp::encode_public(&bob_pubkey, "test");
    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let source = reader::source_from_extrinsic(&hex, 100, 5, 1_700_000_000_000).unwrap();
    assert_eq!(source.sender, alice_sk.public_key());
    assert_eq!(source.at.block().get(), 100);
    assert_eq!(source.at.index().get(), 5);
    assert_eq!(source.timestamp_secs, 1_700_000_000);
}

#[test]
fn extract_block_timestamp_non_timestamp_extrinsic() {
    let hex = format!("0x{}", hex::encode(&[0x10, 0x84, 0x00, 0x00, 0x00, 0x00]));
    let extrinsics = vec![serde_json::Value::String(hex)];
    assert_eq!(reader::extract_block_timestamp(&extrinsics), 0);
}

#[test]
fn extract_block_timestamp_invalid_hex() {
    let extrinsics = vec![serde_json::Value::String("not-hex!!!".into())];
    assert_eq!(reader::extract_block_timestamp(&extrinsics), 0);
}

#[test]
fn extract_block_timestamp_non_string() {
    let extrinsics = vec![serde_json::Value::Number(42.into())];
    assert_eq!(reader::extract_block_timestamp(&extrinsics), 0);
}

#[test]
fn process_remark_public_for_sender_not_recipient() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let charlie_seed = [0xCC; 32];
    let charlie_pubkey = signing(&charlie_seed).public_key();

    // Alice sends to Charlie, but we are Dave — should NOT emit event
    let dave_seed = [0xDD; 32];
    let dave_pubkey = signing(&dave_seed).public_key();

    let remark = samp::encode_public(&charlie_pubkey, "for charlie");
    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&dave_seed);
    let ctx = make_ctx(&keys, &dave_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 100, 0, 0);

    assert!(
        rx.try_recv().is_err(),
        "should not emit for unrelated recipient"
    );
}

#[test]
fn process_remark_public_from_self() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let bob_pubkey = signing(&[0xBB; 32]).public_key();

    let remark = samp::encode_public(&bob_pubkey, "self sent");
    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    // We ARE the sender (alice)
    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&alice_seed);
    let ctx = make_ctx(&keys, &alice_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 100, 0, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::NewMessage { sender, .. }) => {
            assert_eq!(sender, alice_pubkey);
        }
        other => panic!("expected NewMessage, got {:?}", event_debug(&other)),
    }
}

#[test]
fn process_remark_thread_encrypted() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(bob_seed));
    let bob_sk = signing(&bob_seed);
    let bob_pubkey = bob_sk.public_key();

    let nonce = samp::Nonce::from_bytes([0x03; 12]);
    let alice_samp_seed = samp::Seed::from_bytes(alice_seed);

    let thread_content = samp::encode_thread_content(
        taolk::types::BlockRef::ZERO,
        taolk::types::BlockRef::ZERO,
        taolk::types::BlockRef::ZERO,
        b"thread message",
    );
    let plaintext = samp::Plaintext::from_bytes(thread_content);
    let view_tag = samp::compute_view_tag(&alice_samp_seed, &bob_ristretto_pub, &nonce).unwrap();
    let encrypted =
        samp::encrypt(&plaintext, &bob_ristretto_pub, &nonce, &alice_samp_seed).unwrap();
    let remark = samp::encode_encrypted(samp::ContentType::Thread, view_tag, &nonce, &encrypted);

    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 300, 0, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::NewMessage {
            sender,
            content_type,
            decrypted_body,
            ..
        }) => {
            assert_eq!(sender, alice_pubkey);
            assert_eq!(content_type, samp::ContentType::Thread.to_byte());
            assert_eq!(decrypted_body, Some("thread message".to_string()));
        }
        other => panic!(
            "expected NewMessage (thread), got {:?}",
            event_debug(&other)
        ),
    }
}

#[test]
fn process_remark_group_create_and_message() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);

    let bob_seed = [0xBB; 32];
    let bob_pubkey = signing(&bob_seed).public_key();
    let bob_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(bob_seed));

    let alice_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(alice_seed));
    let members = vec![alice_ristretto_pub, bob_ristretto_pub];

    let nonce = samp::Nonce::from_bytes([0x04; 12]);
    let alice_samp_seed = samp::Seed::from_bytes(alice_seed);

    let mut body_bytes = samp::encode_group_members(&members);
    body_bytes.extend_from_slice(b"group hello");
    let plaintext = samp::Plaintext::from_bytes(samp::encode_thread_content(
        taolk::types::BlockRef::ZERO,
        taolk::types::BlockRef::ZERO,
        taolk::types::BlockRef::ZERO,
        &body_bytes,
    ));
    let (eph_pubkey, capsules, ciphertext) =
        samp::encrypt_for_group(&plaintext, &members, &nonce, &alice_samp_seed).unwrap();
    let remark = samp::encode_group(&nonce, &eph_pubkey, &capsules, &ciphertext);

    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 400, 0, 1_700_000_000_000);

    let mut got_group_discovered = false;
    let mut got_group_message = false;
    while let Ok(ev) = rx.try_recv() {
        match ev {
            Event::GroupDiscovered {
                members, group_ref, ..
            } => {
                assert_eq!(members.len(), 2);
                assert_eq!(group_ref, taolk::types::BlockRef::from_parts(400, 0));
                got_group_discovered = true;
            }
            Event::NewGroupMessage {
                body,
                block_number,
                ext_index,
                ..
            } => {
                assert_eq!(body, "group hello");
                assert_eq!(block_number, 400);
                assert_eq!(ext_index, 0);
                got_group_message = true;
            }
            _ => {}
        }
    }
    assert!(got_group_discovered, "should emit GroupDiscovered");
    assert!(got_group_message, "should emit NewGroupMessage");
}

#[test]
fn process_remark_group_followup_message() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);

    let bob_seed = [0xBB; 32];
    let bob_pubkey = signing(&bob_seed).public_key();
    let bob_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(bob_seed));

    let alice_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(alice_seed));
    let members = vec![alice_ristretto_pub, bob_ristretto_pub];

    let nonce = samp::Nonce::from_bytes([0x06; 12]);
    let alice_samp_seed = samp::Seed::from_bytes(alice_seed);

    let group_ref = taolk::types::BlockRef::from_parts(300, 1);
    let plaintext = samp::Plaintext::from_bytes(samp::encode_thread_content(
        group_ref,
        taolk::types::BlockRef::ZERO,
        taolk::types::BlockRef::ZERO,
        b"followup msg",
    ));
    let (eph_pubkey, capsules, ciphertext) =
        samp::encrypt_for_group(&plaintext, &members, &nonce, &alice_samp_seed).unwrap();
    let remark = samp::encode_group(&nonce, &eph_pubkey, &capsules, &ciphertext);

    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&bob_seed);
    let ctx = make_ctx(&keys, &bob_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 500, 2, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::NewGroupMessage {
            body,
            group_ref: wire_ref,
            block_number,
            ext_index,
            ..
        }) => {
            assert_eq!(body, "followup msg");
            assert_eq!(wire_ref, group_ref);
            assert_eq!(block_number, 500);
            assert_eq!(ext_index, 2);
        }
        other => panic!("expected NewGroupMessage, got {:?}", event_debug(&other)),
    }
}

#[test]
fn process_remark_encrypted_outbound_with_seed() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing(&alice_seed);
    let alice_pubkey = alice_sk.public_key();

    let bob_seed = [0xBB; 32];
    let bob_ristretto_pub = samp::public_from_seed(&samp::Seed::from_bytes(bob_seed));

    let plaintext = samp::Plaintext::from_bytes(b"outbound with seed".to_vec());
    let nonce = samp::Nonce::from_bytes([0x07; 12]);
    let alice_samp_seed = samp::Seed::from_bytes(alice_seed);

    let view_tag = samp::compute_view_tag(&alice_samp_seed, &bob_ristretto_pub, &nonce).unwrap();
    let encrypted =
        samp::encrypt(&plaintext, &bob_ristretto_pub, &nonce, &alice_samp_seed).unwrap();
    let remark = samp::encode_encrypted(samp::ContentType::Encrypted, view_tag, &nonce, &encrypted);

    let ext = build_remark_ext(&remark, &alice_sk, 0);
    let hex = ext_to_hex(&ext);

    let (tx, rx) = mpsc::channel();
    let keys = make_keys(&alice_seed); // WITH seed, so should decrypt
    let ctx = make_ctx(&keys, &alice_pubkey, &tx);
    reader::read_extrinsic(&hex, &ctx, 800, 0, 1_700_000_000_000);

    match rx.try_recv() {
        Ok(Event::NewMessage {
            sender,
            decrypted_body,
            ..
        }) => {
            assert_eq!(sender, alice_pubkey);
            assert_eq!(decrypted_body, Some("outbound with seed".to_string()));
        }
        other => panic!("expected NewMessage, got {:?}", event_debug(&other)),
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
