use std::error::Error;
use taolk::error::{AddressError, ChainError, ConfigError, MetadataError, SdkError, WalletError};

#[test]
fn wallet_error_wrong_password_display() {
    assert_eq!(WalletError::WrongPassword.to_string(), "Wrong password");
}

#[test]
fn wallet_error_corrupt_file_display() {
    assert_eq!(
        WalletError::CorruptFile.to_string(),
        "Wallet file is corrupt"
    );
}

#[test]
fn wallet_error_io_wraps_source() {
    let io = std::io::Error::other("disk full");
    let err = WalletError::Io(io);
    assert!(err.to_string().contains("disk full"));
    assert!(err.source().is_some());
}

#[test]
fn address_error_too_short_display() {
    assert_eq!(AddressError::TooShort.to_string(), "address too short");
}

#[test]
fn address_error_bad_checksum_display() {
    assert_eq!(AddressError::BadChecksum.to_string(), "invalid checksum");
}

#[test]
fn address_error_invalid_base58_display() {
    assert_eq!(
        AddressError::InvalidBase58.to_string(),
        "invalid base58 character"
    );
}

#[test]
fn chain_error_connect_carries_payload() {
    let err = ChainError::Connect("ECONNREFUSED".into());
    let s = err.to_string();
    assert!(s.contains("ECONNREFUSED"));
    assert!(s.starts_with("connect:"));
}

#[test]
fn chain_error_send_carries_payload() {
    assert!(ChainError::Send("eof".into()).to_string().contains("eof"));
}

#[test]
fn chain_error_ws_closed_display() {
    assert_eq!(ChainError::WsClosed.to_string(), "ws closed");
}

#[test]
fn chain_error_timeout_display() {
    assert_eq!(
        ChainError::Timeout.to_string(),
        "submission timed out after 60s"
    );
}

#[test]
fn chain_error_bad_length_display() {
    assert_eq!(ChainError::BadLength.to_string(), "expected 32 bytes");
}

#[test]
fn chain_error_missing_field_carries_field_name() {
    let err = ChainError::MissingField("genesis hash");
    assert!(err.to_string().contains("genesis hash"));
}

#[test]
fn chain_error_message_too_long_carries_length() {
    let err = ChainError::MessageTooLong { len: 5_000_000_000 };
    assert!(err.to_string().contains("5000000000"));
}

#[test]
fn chain_error_spec_version_overflow_carries_value() {
    let err = ChainError::SpecVersionOverflow(u64::MAX);
    assert!(err.to_string().contains(&u64::MAX.to_string()));
}

#[test]
fn chain_error_mirror_chain_mismatch_lists_all_three_fields() {
    let err = ChainError::MirrorChainMismatch {
        chain: "Polkadot".into(),
        got: 0,
        expected: 42,
    };
    let s = err.to_string();
    assert!(s.contains("Polkadot"));
    assert!(s.contains("0"));
    assert!(s.contains("42"));
}

#[test]
fn chain_error_hex_wraps_source() {
    let hex_err = hex::decode("zz").unwrap_err();
    let err = ChainError::Hex(hex_err);
    assert!(err.source().is_some());
}

#[test]
fn chain_error_metadata_wraps_source() {
    let inner = MetadataError::Scale("eof".into());
    let err = ChainError::Metadata(inner);
    assert!(err.source().is_some());
}

#[test]
fn metadata_error_type_id_missing_carries_id() {
    assert!(
        MetadataError::TypeIdMissing(7)
            .to_string()
            .contains("type id 7")
    );
}

#[test]
fn metadata_error_non_sequential_lists_both_indices() {
    let err = MetadataError::NonSequential {
        got: 5,
        expected: 3,
    };
    let s = err.to_string();
    assert!(s.contains("5"));
    assert!(s.contains("3"));
}

#[test]
fn metadata_error_shape_lists_context_and_kind() {
    let err = MetadataError::Shape {
        ctx: "AccountInfo",
        kind: "composite",
    };
    let s = err.to_string();
    assert!(s.contains("AccountInfo"));
    assert!(s.contains("composite"));
}

#[test]
fn metadata_error_account_info_short_lists_byte_counts() {
    let err = MetadataError::AccountInfoShort { need: 64, got: 16 };
    let s = err.to_string();
    assert!(s.contains("64"));
    assert!(s.contains("16"));
}

#[test]
fn metadata_error_unknown_typedef_carries_tag() {
    assert!(MetadataError::UnknownTypeDef(99).to_string().contains("99"));
}

#[test]
fn config_error_unknown_key_carries_key() {
    let err = ConfigError::UnknownKey("ui.bogus".into());
    assert!(err.to_string().contains("ui.bogus"));
}

#[test]
fn config_error_invalid_value_lists_expected_and_got() {
    let err = ConfigError::InvalidValue {
        expected: "bool".into(),
        got: "maybe".into(),
    };
    let s = err.to_string();
    assert!(s.contains("bool"));
    assert!(s.contains("maybe"));
}

#[test]
fn config_error_io_wraps_source() {
    let io = std::io::Error::other("permission denied");
    let err = ConfigError::Io(io);
    assert!(err.source().is_some());
}

#[test]
fn sdk_error_wallet_propagates_via_from() {
    let inner = WalletError::WrongPassword;
    let err: SdkError = inner.into();
    assert!(matches!(err, SdkError::Wallet(WalletError::WrongPassword)));
}

#[test]
fn sdk_error_wallet_io_preserves_source_chain_to_io() {
    let io = std::io::Error::other("EACCES");
    let err: SdkError = WalletError::Io(io).into();
    let mut src: Option<&(dyn Error + 'static)> = err.source();
    let mut depth = 0;
    while let Some(s) = src {
        depth += 1;
        if s.to_string().contains("EACCES") {
            return;
        }
        src = s.source();
    }
    panic!("expected EACCES somewhere in source chain (depth {depth})");
}

#[test]
fn sdk_error_chain_propagates_via_from() {
    let inner = ChainError::Timeout;
    let err: SdkError = inner.into();
    assert!(matches!(err, SdkError::Chain(ChainError::Timeout)));
}

#[test]
fn sdk_error_metadata_propagates_via_from() {
    let inner = MetadataError::AccountInfoMissing;
    let err: SdkError = inner.into();
    assert!(matches!(
        err,
        SdkError::Metadata(MetadataError::AccountInfoMissing)
    ));
}

#[test]
fn sdk_error_config_propagates_via_from() {
    let inner = ConfigError::UnknownKey("x".into());
    let err: SdkError = inner.into();
    assert!(matches!(err, SdkError::Config(ConfigError::UnknownKey(_))));
}

#[test]
fn sdk_error_address_propagates_via_from() {
    let inner = AddressError::BadChecksum;
    let err: SdkError = inner.into();
    assert!(matches!(err, SdkError::Address(AddressError::BadChecksum)));
}

#[test]
fn sdk_error_database_via_rusqlite_from() {
    let rusqlite_err = rusqlite::Error::QueryReturnedNoRows;
    let err: SdkError = rusqlite_err.into();
    assert!(matches!(err, SdkError::Database(_)));
}

#[test]
fn sdk_error_encryption_display_does_not_leak_seed_bytes() {
    let err = SdkError::Encryption("aead failed".into());
    let s = err.to_string();
    let known_seed_byte_pattern = "aaaaaaaa";
    assert!(!s.contains(known_seed_byte_pattern));
    assert!(s.contains("aead failed"));
}

#[test]
fn sdk_error_not_found_carries_label() {
    let err = SdkError::NotFound("thread".into());
    assert!(err.to_string().contains("thread"));
}
