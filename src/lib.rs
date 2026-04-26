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

/// Re-export the protocol crate behind the historical `unshell::protocol` path so
/// proc-macro output and downstream code do not need a second migration.
pub use unshell_protocol as protocol;

/// Re-export the leaf library crate behind the historical `unshell::leaves` path
pub use unshell_leaves as leaves;

pub use unshell_macros::{Leaf, Procedure, leaf, procedures};

// pub use ush_obfuscate as obfuscate;
