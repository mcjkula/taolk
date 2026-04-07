fn main() {
    let seed = taolk::secret::Seed::from_bytes([0u8; 32]);
    println!("{seed:?}");
}
