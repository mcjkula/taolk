mod common;

use common::{ALICE_SEED, alice_session};
use taolk::types::{BlockRef, Pubkey};

const BOB_SAMP_SEED: [u8; 32] = [0xBB; 32];

fn bob_scalar() -> curve25519_dalek::scalar::Scalar {
    samp::sr25519_signing_scalar(&samp::Seed::from_bytes(BOB_SAMP_SEED))
}

fn bob_pubkey() -> Pubkey {
    samp::public_from_seed(&samp::Seed::from_bytes(BOB_SAMP_SEED))
}

fn mb(s: &str) -> taolk::types::MessageBody {
    taolk::types::MessageBody::parse(s.to_string()).unwrap()
}

#[test]
fn build_public_message_roundtrip() {
    let session = alice_session();
    let recipient = bob_pubkey();
    let body = mb("hello world");

    let remark = session.build_public_message(&recipient, &body).unwrap();
    let samp::Remark::Public {
        recipient: r,
        body: b,
    } = samp::decode_remark(&remark).unwrap()
    else {
        panic!("expected Public");
    };
    assert_eq!(r, recipient);
    assert_eq!(b.as_str(), "hello world");
}

#[test]
fn build_encrypted_message_decryptable() {
    let session = alice_session();
    let recipient = bob_pubkey();

    let remark = session
        .build_encrypted_message(&ALICE_SEED, &recipient, &mb("hello"))
        .unwrap();
    let samp::Remark::Encrypted(payload) = samp::decode_remark(&remark).unwrap() else {
        panic!("expected Encrypted");
    };

    let plaintext = samp::decrypt(&payload, &bob_scalar()).unwrap();
    assert_eq!(std::str::from_utf8(plaintext.as_bytes()).unwrap(), "hello");
}

#[test]
fn build_thread_root_decryptable() {
    let session = alice_session();
    let recipient = bob_pubkey();

    let remark = session
        .build_thread_root(&ALICE_SEED, &recipient, &mb("thread start"))
        .unwrap();
    let samp::Remark::Thread(payload) = samp::decode_remark(&remark).unwrap() else {
        panic!("expected Thread");
    };

    let plaintext = samp::decrypt(&payload, &bob_scalar()).unwrap();
    let (thread_ref, _reply_to, _continues, body) =
        samp::decode_thread_content(plaintext.as_bytes()).unwrap();

    assert_eq!(thread_ref, BlockRef::ZERO);
    assert_eq!(std::str::from_utf8(body).unwrap(), "thread start");
}

#[test]
fn build_channel_create_roundtrip() {
    let session = alice_session();

    let name = samp::ChannelName::parse("test").unwrap();
    let desc = samp::ChannelDescription::parse("desc").unwrap();
    let remark = session.build_channel_create(&name, &desc).unwrap();
    let samp::Remark::ChannelCreate { name, description } = samp::decode_remark(&remark).unwrap()
    else {
        panic!("expected ChannelCreate");
    };
    assert_eq!(name.as_str(), "test");
    assert_eq!(description.as_str(), "desc");
}

#[test]
fn build_channel_message_roundtrip() {
    let mut session = alice_session();
    let channel_ref = BlockRef::from_parts(500, 1);
    session.subscribe_channel(channel_ref);

    let remark = session.build_channel_message(0, &mb("chan msg")).unwrap();
    let samp::Remark::Channel {
        channel_ref: wire_ref,
        ..
    } = samp::decode_remark(&remark).unwrap()
    else {
        panic!("expected Channel");
    };
    assert_eq!(wire_ref, channel_ref);
}

#[test]
fn build_group_create_decryptable() {
    let session = alice_session();
    let alice_pk = samp::public_from_seed(&samp::Seed::from_bytes(ALICE_SEED));
    let bob_pk = samp::public_from_seed(&samp::Seed::from_bytes(BOB_SAMP_SEED));
    let members = vec![alice_pk, bob_pk];

    let remark = session
        .build_group_create(&ALICE_SEED, &members, &mb("group hello"))
        .unwrap();
    let samp::Remark::Group(payload) = samp::decode_remark(&remark).unwrap() else {
        panic!("expected Group");
    };

    let plaintext =
        samp::decrypt_from_group(&payload.content, &bob_scalar(), &payload.nonce, Some(2)).unwrap();

    let (_group_ref, _reply_to, _continues, body) =
        samp::decode_thread_content(plaintext.as_bytes()).unwrap();

    let (member_list, text) = samp::decode_group_members(body).unwrap();
    assert_eq!(member_list.len(), 2);
    assert!(member_list.contains(&alice_pk));
    assert!(member_list.contains(&bob_pk));
    assert_eq!(std::str::from_utf8(text).unwrap(), "group hello");
}

#[test]
fn build_returns_error_for_invalid_thread_idx() {
    let session = alice_session();
    let result = session.build_thread_reply(&ALICE_SEED, 999, &mb("test"));
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not found"), "expected NotFound, got: {err}");
}

#[test]
fn build_returns_error_for_invalid_channel_idx() {
    let session = alice_session();
    let result = session.build_channel_message(999, &mb("test"));
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not found"), "expected NotFound, got: {err}");
}

#[test]
fn build_returns_error_for_invalid_group_idx() {
    let session = alice_session();
    let result = session.build_group_message(&ALICE_SEED, 999, &mb("test"));
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("not found"), "expected NotFound, got: {err}");
}
