//! # UnShell Core
//!
//! This crate implements the UnShell protocol as a pure, `no_std` library.
//! It provides routed endpoint communication using an explicit tree topology.
//!
//! ## Architecture
//!
//! - [`protocol`] - Wire types, framing, stateless validation, and routing/runtime.
//!
//! The library requires `alloc` for path and payload management.

#![no_std]

pub extern crate alloc;
// Re-export derive macros against a stable `::unshell` path, including when the
// macros are used inside this crate's own examples and tests.
#[allow(unused_extern_crates)]
extern crate self as unshell;

pub mod logger;
pub mod protocol;

pub use unshell_macros::Leaf;

// pub use ush_obfuscate as obfuscate;
