//! Entry point for the `UniFFI` binding generator.
//!
//! Build with:
//! ```sh
//! cargo build -p brega-uniffi --features cli --bin uniffi-bindgen
//! ```
//!
//! Generate Swift bindings:
//! ```sh
//! cargo run -p brega-uniffi --features cli --bin uniffi-bindgen -- \
//!     generate --library target/debug/libbrega_uniffi.dylib \
//!     --language swift \
//!     --out-dir bindings/swift/Sources/BregaFFI
//! ```

fn main() {
    uniffi::uniffi_bindgen_main();
}
