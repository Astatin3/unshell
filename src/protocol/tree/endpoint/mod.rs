//! Endpoint runtime and traits.
//!
//! This module provides the core logic for a protocol endpoint, including
//! packet ingress, routing decisions, and hook lifecycle management.
//!
//! Protocol section mapping:
//! - `builders`: packet construction and outbound hook declaration
//! - `receive`: framed ingress, authority checks, and route selection
//! - `hooks`: hook lifecycle, peer validation, and fault emission
//! - `introspection`: reserved empty-procedure discovery responses
//! - `core`: externally visible endpoint state and result types

mod builders;
mod core;
mod hooks;
mod introspection;
mod receive;

pub use core::{
    ChildRoute, ConnectionState, Endpoint, EndpointError, EndpointOutcome, Ingress,
    LeafBehavior, LeafSpec, LocalEvent, ProtocolEndpoint,
};
