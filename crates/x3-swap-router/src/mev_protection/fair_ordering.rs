//! The commit-reveal ordering lane, re-exported.
//!
//! The algorithm moved to the `x3-order-window` crate on 2026-09-26: it only ever needed
//! `sp_core::hashing::blake2_256`, `H160`/`H256`, `sp_std`, and a no_std-capable `serde`, so it
//! now compiles inside a runtime pallet and a chain path can enforce the order this crate's tests
//! verify. There is still one implementation — this module is a re-export, not a copy, and the 26
//! tests that used to live here moved with it (`cargo test -p x3-order-window`).

pub use x3_order_window::*;
