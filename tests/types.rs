use taolk::types::{BlockRef, Pubkey};

#[test]
fn pubkey_equality() {
    let a = Pubkey::from_bytes([0xAA; 32]);
    let b = Pubkey::from_bytes([0xAA; 32]);
    assert_eq!(a, b);
}

#[test]
fn pubkey_inequality() {
    let a = Pubkey::from_bytes([0xAA; 32]);
    let b = Pubkey::from_bytes([0xBB; 32]);
    assert_ne!(a, b);
}

#[test]
fn pubkey_zero() {
    assert_eq!(Pubkey::ZERO.as_bytes(), &[0u8; 32]);
}

#[test]
fn pubkey_as_bytes() {
    let pk = Pubkey::from_bytes([0xCC; 32]);
    assert_eq!(pk.as_bytes(), &[0xCC; 32]);
}

#[test]
fn pubkey_from_bytes_round_trip() {
    let pk = Pubkey::from_bytes([1u8; 32]);
    assert_eq!(pk.as_bytes(), &[1u8; 32]);
}

#[test]
fn pubkey_into_bytes() {
    let pk = Pubkey::from_bytes([0xDD; 32]);
    let bytes: [u8; 32] = pk.into_bytes();
    assert_eq!(bytes, [0xDD; 32]);
}

#[test]
fn blockref_zero() {
    assert_eq!(BlockRef::ZERO.block, 0);
    assert_eq!(BlockRef::ZERO.index, 0);
}

#[test]
fn blockref_is_zero() {
    assert!(BlockRef::ZERO.is_zero());
    assert!(!BlockRef { block: 1, index: 0 }.is_zero());
}

#[test]
fn blockref_ordering() {
    let a = BlockRef { block: 0, index: 5 };
    let b = BlockRef { block: 1, index: 0 };
    assert!(b > a);

    let c = BlockRef { block: 1, index: 0 };
    let d = BlockRef { block: 1, index: 3 };
    assert!(d > c);
}

#[test]
fn blockref_hash_consistent() {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let a = BlockRef {
        block: 42,
        index: 7,
    };
    let b = BlockRef {
        block: 42,
        index: 7,
    };

    let mut h1 = DefaultHasher::new();
    a.hash(&mut h1);
    let mut h2 = DefaultHasher::new();
    b.hash(&mut h2);
    assert_eq!(h1.finish(), h2.finish());
}
