//! # UnShell Protocol
//!
//! The protocol crate owns the wire types, framing, validation helpers, and the
//! small tree runtime used by endpoint implementations.

#![no_std]

pub extern crate alloc;
#[allow(unused_extern_crates)]
extern crate self as unshell;

/// Keep the historical nested path so existing imports and proc-macro output can
/// continue to target `unshell::protocol::...` while the implementation lives in
/// its own crate.
pub mod protocol;

pub use protocol::*;

#[cfg(test)]
pub use unshell_macros::{Leaf, Procedure, procedures};
