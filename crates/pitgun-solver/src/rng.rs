//! Compatibility facade for deterministic random primitives.
//!
//! New code should import these primitives from [`pitgun_runtime::rng`]. This
//! module remains temporarily available so existing Racing and WASM consumers
//! can migrate without changing deterministic evidence.

pub use pitgun_runtime::rng::*;
