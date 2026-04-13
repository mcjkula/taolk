#[test]
fn forbidden_traits_on_secret_types_fail_to_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/*.rs");
}
