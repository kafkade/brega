# xtask

Internal `cargo xtask` runner for the brega workspace. Not published.

Run with `cargo xtask <subcommand>` (alias set up in `.cargo/config.toml`).

## Subcommands

### `xcframework`

Regenerates the UniFFI Swift bindings and builds the Apple `BregaFFI.xcframework`
consumed by `bindings/swift`. Requires macOS with Xcode (`xcodebuild`, `lipo`)
and `rustup`.

```bash
cargo xtask xcframework
```

Pipeline:

1. `rustup target add` the four Apple targets (idempotent):
   `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `aarch64-apple-darwin`,
   `x86_64-apple-darwin`.
2. `cargo build --profile apple-ffi -p brega-uniffi` for each target. The
   dedicated `apple-ffi` profile (in the root `Cargo.toml`) inherits `release`
   but disables `strip` and `lto`, which would otherwise remove the
   `#[no_mangle]` UniFFI scaffolding from the static library.
3. Run `uniffi-bindgen` against the macOS dylib and copy the generated
   `brega_uniffi.swift` into `bindings/swift/Sources/BregaFFI/`.
4. `lipo` the two macOS static-lib slices into a universal slice.
5. `xcodebuild -create-xcframework` over the macOS, iOS-device, and
   iOS-simulator slices → `bindings/swift/BregaFFI.xcframework`.

Both `brega_uniffi.swift` and `BregaFFI.xcframework/` are gitignored build
outputs. See `bindings/swift/README.md` and ADR-005.
