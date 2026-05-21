//! C ABI bridge for the LP-0017 Basecamp Qt6 plugin.
//!
//! Exposes `index_batch`, `init_registry`, `lookup` as JSON-in / JSON-out
//! `extern "C"` functions so the Qt module can call into a stable
//! interface without depending on the Rust toolchain.
//!
//! The concrete bindings land in a later commit; this file is the
//! placeholder so the workspace builds.
