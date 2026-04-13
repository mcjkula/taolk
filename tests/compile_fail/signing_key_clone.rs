fn main() {
    let seed = taolk::secret::Seed::from_bytes([0u8; 32]);
    let signing = seed.derive_signing_key();
    let _ = signing.clone();
}
