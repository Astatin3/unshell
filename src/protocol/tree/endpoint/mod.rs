//! Protocol-tree endpoint runtime.
//!
//! This module holds the state machine that validates ingress, decides whether a
//! packet should be handled locally or forwarded, and manages hook lifetimes for
//! call/data/fault exchanges.

mod builders;
mod core;
mod hooks;
mod introspection;
mod receive;

pub use core::{
    ChildRoute, Endpoint, EndpointError, EndpointOutcome, Ingress, LeafSpec, LocalEvent,
    ProtocolEndpoint,
};
