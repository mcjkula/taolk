use taolk::error::MetadataError;
use taolk::metadata::Metadata;

#[test]
fn from_runtime_metadata_rejects_empty_input() {
    let err = Metadata::from_runtime_metadata(&[]).unwrap_err();
    assert!(matches!(err, MetadataError::Scale(_)));
}

#[test]
fn from_runtime_metadata_rejects_wrong_magic() {
    // Valid 4-byte u32 prefix but not METADATA_MAGIC ("meta" little-endian).
    let bytes = [0x00, 0x00, 0x00, 0x00, 14u8];
    let err = Metadata::from_runtime_metadata(&bytes).unwrap_err();
    let s = err.to_string();
    assert!(s.contains("magic"), "expected magic mismatch, got: {s}");
}

#[test]
fn from_runtime_metadata_rejects_wrong_version() {
    // Correct magic "meta" + version byte != 14.
    let mut bytes = vec![0x6du8, 0x65, 0x74, 0x61];
    bytes.push(13);
    let err = Metadata::from_runtime_metadata(&bytes).unwrap_err();
    let s = err.to_string();
    assert!(s.contains("version"), "expected version error, got: {s}");
    assert!(s.contains("13"));
}

#[test]
fn from_runtime_metadata_rejects_truncated_after_magic() {
    let bytes = vec![0x6du8, 0x65, 0x74, 0x61];
    let err = Metadata::from_runtime_metadata(&bytes).unwrap_err();
    assert!(matches!(err, MetadataError::Scale(_)));
}

#[test]
fn from_runtime_metadata_rejects_truncated_in_registry() {
    // magic + version + truncated SCALE compact length prefix (incomplete)
    let bytes = vec![0x6du8, 0x65, 0x74, 0x61, 14];
    let err = Metadata::from_runtime_metadata(&bytes).unwrap_err();
    assert!(matches!(err, MetadataError::Scale(_)));
}
