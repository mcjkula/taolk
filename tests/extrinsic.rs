mod common;

use common::signing_from_seed;

#[test]
fn build_remark_ext_round_trip() {
    let seed = [0xAA; 32];
    let sk = signing_from_seed(&seed);
    let remark = samp::encode_public(&samp::Pubkey::from_bytes([0xBB; 32]), "hello");
    let ext = common::build_remark_ext(&remark, &sk, 0);
    assert!(!ext.as_bytes().is_empty());

    let call = samp::extrinsic::extract_call(&ext).unwrap();
    let pair = (call.pallet().get(), call.call().get());
    assert_eq!(pair, (0, 7));
}

#[test]
fn build_remark_ext_different_nonces_differ() {
    let seed = [0xAA; 32];
    let sk = signing_from_seed(&seed);
    let remark = samp::encode_public(&samp::Pubkey::from_bytes([0xBB; 32]), "hello");
    let ext0 = common::build_remark_ext(&remark, &sk, 0);
    let ext1 = common::build_remark_ext(&remark, &sk, 1);
    assert_ne!(ext0.as_bytes(), ext1.as_bytes());
}

#[test]
fn build_remark_ext_extracts_signer() {
    let seed = [0xAA; 32];
    let sk = signing_from_seed(&seed);
    let expected_pubkey = sk.public_key();
    let remark = samp::encode_public(&samp::Pubkey::from_bytes([0xBB; 32]), "test");
    let ext = common::build_remark_ext(&remark, &sk, 0);
    let signer = samp::extrinsic::extract_signer(&ext).unwrap();
    assert_eq!(signer, expected_pubkey);
}

#[test]
fn chain_info_construction() {
    let ci = common::test_chain_info();
    assert_eq!(ci.name.as_str(), "Test");
    assert_eq!(ci.ss58_prefix, samp::Ss58Prefix::SUBSTRATE_GENERIC);
}

#[test]
fn chain_info_clone() {
    let ci = common::test_chain_info();
    let ci2 = ci.clone();
    assert_eq!(ci.name.as_str(), ci2.name.as_str());
}

// SYSTEM_REMARK and SYSTEM_REMARK_WITH_EVENT are pub(crate), tested inline

#[test]
fn build_remark_ext_large_payload() {
    let seed = [0xAA; 32];
    let sk = signing_from_seed(&seed);
    let body = "x".repeat(1000);
    let remark = samp::encode_public(&samp::Pubkey::from_bytes([0xBB; 32]), &body);
    let ext = common::build_remark_ext(&remark, &sk, 0);
    assert!(ext.as_bytes().len() > 1000);
}

#[test]
fn build_remark_ext_encrypted() {
    let alice_seed = [0xAA; 32];
    let alice_sk = signing_from_seed(&alice_seed);

    let bob_seed = samp::Seed::from_bytes([0xBB; 32]);
    let bob_pub = samp::public_from_seed(&bob_seed);

    let nonce = samp::Nonce::from_bytes([0x01; 12]);
    let sender = samp::Seed::from_bytes(alice_seed);
    let plaintext = samp::Plaintext::from_bytes(b"encrypted test".to_vec());
    let view_tag = samp::compute_view_tag(&sender, &bob_pub, &nonce).unwrap();
    let ciphertext = samp::encrypt(&plaintext, &bob_pub, &nonce, &sender).unwrap();
    let remark =
        samp::encode_encrypted(samp::ContentType::Encrypted, view_tag, &nonce, &ciphertext);

    let ext = common::build_remark_ext(&remark, &alice_sk, 42);
    let signer = samp::extrinsic::extract_signer(&ext).unwrap();
    assert_eq!(signer, alice_sk.public_key());
}

#[test]
fn build_remark_ext_channel_create() {
    let seed = [0xAA; 32];
    let sk = signing_from_seed(&seed);
    let name = samp::ChannelName::parse("test-channel").unwrap();
    let desc = samp::ChannelDescription::parse("a test").unwrap();
    let remark = samp::encode_channel_create(&name, &desc);
    let ext = common::build_remark_ext(&remark, &sk, 0);
    assert!(!ext.as_bytes().is_empty());
}
