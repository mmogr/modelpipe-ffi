//! The bindings generator, as a binary inside the crate it binds.
//!
//! `UniFFI`'s documented shape, and the reason for it is version skew: the
//! generated Swift has to match the scaffolding compiled into the static
//! library exactly. A separately-installed `uniffi-bindgen` is a second
//! version to keep in step by hand, and a mismatch shows up as a link error
//! against a symbol nobody wrote. Built from this crate's own `uniffi`
//! dependency, the two cannot disagree.
//!
//! Run by `make swift`; not shipped in the `XCFramework`.

fn main() {
    uniffi::uniffi_bindgen_main();
}
