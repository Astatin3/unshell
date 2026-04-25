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

extern crate alloc;

pub mod logger;
pub mod protocol;

// pub use ush_obfuscate as obfuscate;
