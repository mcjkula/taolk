fn main() {
    let p = taolk::secret::Password::new("hunter2".to_string());
    let _display: &dyn std::fmt::Display = &p;
}
