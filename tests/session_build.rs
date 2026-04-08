mod common;

use common::{ALICE_SEED, alice_session};
use taolk::types::{BlockRef, Pubkey};

const BOB_SAMP_SEED: [u8; 32] = [0xBB; 32];

fn bob_scalar() -> curve25519_dalek::scalar::Scalar {
    samp::sr25519_signing_scalar(&BOB_SAMP_SEED)
}

fn bob_pubkey() -> Pubkey {
    Pubkey(samp::public_from_seed(&BOB_SAMP_SEED))
}

#[test]
fn build_public_message_roundtrip() {
    let session = alice_session();
    let recipient = bob_pubkey();
    let body = "hello world";

    let remark = session.build_public_message(&recipient, body).unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();

    assert_eq!(decoded.content_type, samp::ContentType::Public);
    assert_eq!(decoded.recipient, recipient.0);
    assert_eq!(std::str::from_utf8(&decoded.content).unwrap(), body);
}

#[test]
fn build_encrypted_message_decryptable() {
    let session = alice_session();
    let recipient = bob_pubkey();

    let remark = session
        .build_encrypted_message(&recipient, "hello")
        .unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();

    assert_eq!(decoded.content_type, samp::ContentType::Encrypted);

    let plaintext = samp::decrypt(&decoded, &bob_scalar()).unwrap();
    assert_eq!(std::str::from_utf8(&plaintext).unwrap(), "hello");
}

#[test]
fn build_thread_root_decryptable() {
    let session = alice_session();
    let recipient = bob_pubkey();

    let remark = session
        .build_thread_root(&recipient, "thread start")
        .unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();

    assert_eq!(decoded.content_type, samp::ContentType::Thread);

    let plaintext = samp::decrypt(&decoded, &bob_scalar()).unwrap();
    let (thread_ref, _reply_to, _continues, body) =
        samp::decode_thread_content(&plaintext).unwrap();

    assert_eq!(thread_ref, BlockRef::ZERO);
    assert_eq!(std::str::from_utf8(body).unwrap(), "thread start");
}

#[test]
fn build_channel_create_roundtrip() {
    let session = alice_session();

    let remark = session.build_channel_create("test", "desc").unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();

    assert_eq!(decoded.content_type, samp::ContentType::ChannelCreate);

    let (name, description) = samp::decode_channel_create(&decoded.content).unwrap();
    assert_eq!(name, "test");
    assert_eq!(description, "desc");
}

#[test]
fn build_channel_message_roundtrip() {
    let mut session = alice_session();
    let channel_ref = BlockRef {
        block: 500,
        index: 1,
    };
    session.subscribe_channel(channel_ref);

    let remark = session.build_channel_message(0, "chan msg").unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();

    assert_eq!(decoded.content_type, samp::ContentType::Channel);

    let wire_ref = samp::channel_ref_from_recipient(&decoded.recipient);
    assert_eq!(wire_ref, channel_ref);
}

#[test]
fn build_group_create_decryptable() {
    let session = alice_session();
    let alice_pk = samp::public_from_seed(&ALICE_SEED);
    let bob_pk = samp::public_from_seed(&BOB_SAMP_SEED);
    let members = vec![Pubkey(alice_pk), Pubkey(bob_pk)];

    let remark = session.build_group_create(&members, "group hello").unwrap();
    let decoded = samp::decode_remark(&remark).unwrap();

    assert_eq!(decoded.content_type, samp::ContentType::Group);

    let plaintext =
        samp::decrypt_from_group(&decoded.content, &bob_scalar(), &decoded.nonce, Some(2)).unwrap();

    let (_group_ref, _reply_to, _continues, body) =
        samp::decode_thread_content(&plaintext).unwrap();

    let (member_list, text) = samp::decode_group_members(body).unwrap();
    assert_eq!(member_list.len(), 2);
    assert!(member_list.contains(&alice_pk));
    assert!(member_list.contains(&bob_pk));
    assert_eq!(std::str::from_utf8(text).unwrap(), "group hello");
}

#[test]
fn build_returns_error_for_invalid_thread_idx() {
    let session = alice_session();
    let result = session.build_thread_reply(999, "test");
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not found"), "expected NotFound, got: {err}");
}

#[test]
fn build_returns_error_for_invalid_channel_idx() {
    let session = alice_session();
    let result = session.build_channel_message(999, "test");
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not found"), "expected NotFound, got: {err}");
}

#[test]
fn build_returns_error_for_invalid_group_idx() {
    let session = alice_session();
    let result = session.build_group_message(999, "test");
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not found"), "expected NotFound, got: {err}");
}
