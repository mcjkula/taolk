use std::fmt::{Debug, Display};
use taolk::secret::{Password, Seed, SigningKey};

#[test]
fn forbidden_traits_on_secret_types_fail_to_compile() {
    static_assertions::assert_not_impl_any!(Password: Clone, Debug, Display);
    static_assertions::assert_not_impl_any!(Seed: Clone, Debug, Display);
    static_assertions::assert_not_impl_any!(SigningKey: Clone, Debug, Display);
}
