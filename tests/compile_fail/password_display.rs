fn main() {
    let p = taolk::secret::Password::new("hunter2".to_string());
    println!("{p}");
}
