# third_party

Patched copies of upstream crates. Each keeps its original licence file.

## apple-cryptokit-rs 0.2.1 (MIT, https://crates.io/crates/apple-cryptokit-rs)

Pulled in by `str0m-apple-crypto` (WebRTC/WHIP streaming crypto). The only change is in
`build.rs`, which now passes `--build-system native` to `swift build`. Swift 6.4 defaults to
the "swiftbuild" system, which drops the crate's internal `@_cdecl` functions (the symbols
Rust links against) and writes the library to a different folder, so linking failed. The
native system is deprecated in SwiftPM, so this is a stopgap.

Remove this copy and the `[patch.crates-io]` entry in the root `Cargo.toml` once upstream
ships a fix.
