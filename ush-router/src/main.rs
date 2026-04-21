//! # ush-router — UnShell Router Binary
//!
//! The router accepts TCP connections from all node types (payloads, operators)
//! and routes packets between them based on path-prefix matching.
//!
//! ## Usage
//!
//! ```text
//! ush-router --bind 0.0.0.0:9000
//! ```
//!
//! ## Architecture
//!
//! ```text
//! main thread
//!   └─ TcpListener loop
//!        └─ for each incoming connection:
//!             spawn node_thread(TcpStream)
//!
//! node_thread
//!   1. Read HandshakeMessage → register in NodeRegistry
//!   2. Send HandshakeAck
//!   3. recv loop:
//!        Read PacketHeader + payload
//!        Look up dst_path in NodeRegistry
//!        If found: forward raw bytes to that node's channel
//!        If not found: send NoBranchError response to src_path
//!   4. On disconnect: remove from NodeRegistry
//!
//! write_thread (per node)
//!   Receives bytes from channel → writes to TcpStream
//! ```

mod node;
mod registry;
mod router;

fn main() {
    // TODO: parse --bind argument
    let bind_addr = "0.0.0.0:9000";
    router::run(bind_addr).expect("router failed");
}
