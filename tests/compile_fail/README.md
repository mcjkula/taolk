# Compile-fail anchors

Each `.rs` file in this directory is a tiny program that **must fail to
compile** with the exact error in its sibling `.stderr` snapshot. The
purpose is to lock in compile-time guarantees about taolk's secret types:

| File | Anchor |
|---|---|
| `seed_clone.rs` | `Seed` does not implement `Clone` |
| `seed_debug.rs` | `Seed` does not implement `Debug` |
| `signing_key_clone.rs` | `SigningKey` does not implement `Clone` |
| `password_display.rs` | `Password` does not implement `Display` |

If a Rust toolchain upgrade changes the compiler error wording and these
tests start failing, regenerate the snapshots:

```sh
TRYBUILD=overwrite cargo test --test compile_fail
```

Review the diff and commit if the new wording is reasonable.
