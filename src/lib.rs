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
#![feature(const_index)]
#![feature(const_trait_impl)]

pub extern crate alloc;

mod hash;
pub mod logger;

pub mod protocol {
    pub use unshell_protocol::*;

    pub use unshell_macros::unshell_leaf;
}

pub use hash::hash;
