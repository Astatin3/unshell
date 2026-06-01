//! Minimal TCP transport leaves for adjacent UnShell endpoints.
//!
//! This crate deliberately stays small: it does not own an [`unshell::protocol::Endpoint`]
//! or run a scheduler. Callers keep their endpoint and application leaves, then tick a
//! TCP leaf to move serialized packets between the endpoint's outbound queues and a
//! nonblocking socket.

use unshell::crypto::hash_str_32;

mod client;
mod server;
mod transport;

pub use client::TCPClientLeaf;
pub use server::TCPServerLeaf;

macro_rules! version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

/// Stable interface identifier for the listening TCP bridge leaf.
pub const IDENTIFIER_SERVER: &str = concat!("dev.unshell.", version!(), ".tcp_simple.server");

/// Numeric identifier for [`TCPServerLeaf`].
pub const IDENTIFIER_SERVER_HASH: u32 = hash_str_32(IDENTIFIER_SERVER);

/// Stable interface identifier for the connecting TCP bridge leaf.
pub const IDENTIFIER_CLIENT: &str = concat!("dev.unshell.", version!(), ".tcp_simple.client");

/// Numeric identifier for [`TCPClientLeaf`].
pub const IDENTIFIER_CLIENT_HASH: u32 = hash_str_32(IDENTIFIER_CLIENT);
