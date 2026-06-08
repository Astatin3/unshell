//! # UnShell Core
//!
//! This crate implements the UnShell protocol as a pure, `no_std` library.
//! It provides routed endpoint communication using an explicit tree topology.
//!
//! ## Architecture
//!
//! - [`protocol`] - Wire types, framing, stateless validation, and routing/runtime.
//! - [`interface`] - Namespaced blob storage and frontend services for optional
//!   operator interfaces.
//!
//! The library requires `alloc` for path and payload management.

#![no_std]

pub extern crate alloc;

pub mod crypto;
pub mod interface;
pub mod logger;
pub mod protocol;

// pub use hash::hash;
