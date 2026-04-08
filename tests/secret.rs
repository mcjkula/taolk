use taolk::secret::{Password, Phrase, Seed};

#[test]
fn seed_from_phrase_zero_vector_is_wire_format_stable() {
    let phrase = Phrase::parse(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
    )
    .unwrap();
    let seed = Seed::from_phrase(&phrase);
    let expected: [u8; 32] =
        hex::decode("4ed8d4b17698ddeaa1f1559f152f87b5d472f725ca86d341bd0276f1b61197e2")
            .unwrap()
            .try_into()
            .unwrap();
    assert_eq!(seed.as_bytes(), &expected);
}

#[test]
fn signing_key_public_key_matches_seed_derivation_across_calls() {
    let seed = Seed::from_bytes([0x77; 32]);
    let sk1 = seed.derive_signing_key();
    let sk2 = seed.derive_signing_key();
    assert_eq!(sk1.public_key(), sk2.public_key());
}

#[test]
fn signing_key_different_seeds_produce_different_public_keys() {
    let a = Seed::from_bytes([0x01; 32]).derive_signing_key();
    let b = Seed::from_bytes([0x02; 32]).derive_signing_key();
    assert_ne!(a.public_key(), b.public_key());
}

#[test]
fn ct_eq_returns_true_for_identical_seeds() {
    let a = Seed::from_bytes([0x42; 32]);
    let b = Seed::from_bytes([0x42; 32]);
    assert!(a.ct_eq(&b));
}

#[test]
fn ct_eq_returns_false_for_one_byte_difference() {
    let a = Seed::from_bytes([0x42; 32]);
    let mut bytes = [0x42u8; 32];
    bytes[15] = 0x00;
    let b = Seed::from_bytes(bytes);
    assert!(!a.ct_eq(&b));
}

#[test]
fn ct_eq_returns_false_for_completely_different_seeds() {
    let a = Seed::from_bytes([0x00; 32]);
    let b = Seed::from_bytes([0xFF; 32]);
    assert!(!a.ct_eq(&b));
}

#[test]
fn password_value_does_not_escape_outer_scope() {
    {
        let p = Password::new("secret-value-123".to_string());
        assert_eq!(p.as_str(), "secret-value-123");
    }
}

#[test]
fn phrase_round_trips_through_parse_and_words() {
    let original = Phrase::generate().unwrap();
    let words_str = original.words().to_string();
    let reparsed = Phrase::parse(&words_str).unwrap();
    assert_eq!(original.words(), reparsed.words());
}
