//! # Node Thread
//!
//! Each connected node runs in its own thread. The node thread:
//!
//! 1. Reads a `HandshakeMessage` from the new connection.
//! 2. Registers the node in the `NodeRegistry`.
//! 3. Sends a `HandshakeAck` back.
//! 4. Enters the recv loop:
//!    - Read packet (header + payload raw bytes).
//!    - Look up `dst_path` in the registry.
//!    - If found: forward raw framed bytes to that node's channel.
//!    - If not found: send a `NoBranchError` response to the sender.
//! 5. On disconnect: unregister the node and exit.
//!
//! ## Write thread
//!
//! A separate write-thread per node reads from the channel and writes to
//! the `TcpStream`. This decouples the recv loop from potentially slow sends
//! (e.g., a slow operator connection should not block a payload recv loop).
//!
//! ```text
//! node_thread (recv)
//!   reads from TcpStream
//!   forwards to registry-lookup → channel
//!
//! write_thread
//!   reads from channel
//!   writes to TcpStream
//! ```

use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use std::thread;

use crossbeam_channel::{unbounded, Receiver, Sender};
use unshell::protocol::{
    HandshakeAck, HandshakeMessage,
    PacketHeader, PacketType, ResponseStatus, TreeResponse,
    content,
};
use unshell::transport::tcp::TcpTransport;
use unshell::transport::Transport;

use crate::registry::{NodeEntry, NodeRegistry};

/// Time allowed for the connecting node to send its `HandshakeMessage`.
const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Spawn a node thread (and its associated write-thread) for a new connection.
///
/// # Arguments
///
/// * `stream` — the accepted TCP stream for this node.
/// * `registry` — shared node registry (wrapped in `Arc<Mutex>`).
pub fn spawn_node(stream: TcpStream, registry: Arc<Mutex<NodeRegistry>>) {
    thread::spawn(move || {
        // Set the handshake timeout on the stream.
        if let Err(e) = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT)) {
            eprintln!("[router] failed to set handshake timeout: {e}");
            return;
        }

        let mut transport = TcpTransport::from_stream(stream);

        // --- Handshake ---
        let handshake = match receive_handshake(&mut transport) {
            Ok(hs) => hs,
            Err(e) => {
                eprintln!("[router] handshake failed: {e}");
                return;
            }
        };

        let node_id = handshake.node_id.clone();
        eprintln!(
            "[router] node connected: id={} type={:?} paths={:?}",
            node_id, handshake.node_type, handshake.registered_paths
        );

        // Check for duplicate node_id
        {
            let reg = registry.lock().expect("registry lock poisoned");
            if reg.node_list().iter().any(|n| n.node_id == node_id) {
                let ack = HandshakeAck {
                    accepted: false,
                    assigned_base_path: String::new(),
                    rejection_reason: Some("duplicate_node_id".into()),
                };
                let _ = send_handshake_ack(&mut transport, &node_id, &ack);
                return;
            }
        }

        // Create a channel for the write-thread
        let (tx, rx): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = unbounded();

        // Register the node
        let assigned_path = handshake
            .registered_paths
            .first()
            .cloned()
            .unwrap_or_else(|| format!("/{}", node_id));

        let connected_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        {
            let mut reg = registry.lock().expect("registry lock poisoned");
            reg.register(NodeEntry {
                node_id: node_id.clone(),
                node_type: handshake.node_type,
                registered_paths: handshake.registered_paths,
                connected_at,
                tx,
            });
        }

        // Send ack
        let ack = HandshakeAck {
            accepted: true,
            assigned_base_path: assigned_path,
            rejection_reason: None,
        };
        if let Err(e) = send_handshake_ack(&mut transport, &node_id, &ack) {
            eprintln!("[router] failed to send ack to {node_id}: {e}");
            let mut reg = registry.lock().expect("registry lock poisoned");
            reg.unregister(&node_id);
            return;
        }

        // Remove the read timeout for the main recv loop
        if let Err(e) = transport.stream_ref().set_read_timeout(None) {
            eprintln!("[router] failed to clear read timeout: {e}");
        }

        // Spawn the write-thread
        // Clone the stream via try_clone so the write-thread has its own handle.
        let write_stream = match transport.stream_ref().try_clone() {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[router] failed to clone stream for write-thread: {e}");
                let mut reg = registry.lock().expect("registry lock poisoned");
                reg.unregister(&node_id);
                return;
            }
        };
        let write_node_id = node_id.clone();
        thread::spawn(move || {
            write_loop(write_stream, rx, &write_node_id);
        });

        // --- Main recv loop ---
        recv_loop(&mut transport, &node_id, &registry);

        // Cleanup
        eprintln!("[router] node disconnected: {node_id}");
        let mut reg = registry.lock().expect("registry lock poisoned");
        reg.unregister(&node_id);
    });
}

// ---------------------------------------------------------------------------
// Recv loop
// ---------------------------------------------------------------------------

/// Read packets from this node and route them to the appropriate destination.
fn recv_loop(
    transport: &mut TcpTransport,
    source_node_id: &str,
    registry: &Arc<Mutex<NodeRegistry>>,
) {
    loop {
        let (header, payload) = match transport.recv() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[router] recv error from {source_node_id}: {e}");
                break;
            }
        };

        // Build the raw framed bytes to forward
        let raw = match encode_raw_packet(&header, &payload) {
            Some(b) => b,
            None => {
                eprintln!("[router] failed to re-encode packet from {source_node_id}");
                continue;
            }
        };

        // Look up destination
        let route_result = {
            let reg = registry.lock().expect("registry lock poisoned");
            reg.find_route(&header.dst_path).map(|tx| tx.clone())
        };

        match route_result {
            Some(tx) => {
                if tx.send(raw).is_err() {
                    // Destination's write-thread has exited — the node
                    // probably disconnected. Send a NoBranchError back.
                    eprintln!(
                        "[router] destination channel dead for path {}",
                        header.dst_path
                    );
                    send_no_branch_error(transport, source_node_id, &header);
                }
            }
            None => {
                eprintln!(
                    "[router] no route for path {} (from {})",
                    header.dst_path, source_node_id
                );
                send_no_branch_error(transport, source_node_id, &header);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Write loop
// ---------------------------------------------------------------------------

/// Receive bytes from the channel and write them to the node's `TcpStream`.
///
/// Runs in a dedicated thread per node. Exits when the channel is disconnected
/// (which happens when the node is unregistered from the registry).
fn write_loop(mut stream: TcpStream, rx: Receiver<Vec<u8>>, node_id: &str) {
    use std::io::Write;
    for bytes in &rx {
        if let Err(e) = stream.write_all(&bytes) {
            eprintln!("[router] write error to {node_id}: {e}");
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read and deserialise the `HandshakeMessage` from a new connection.
fn receive_handshake(
    transport: &mut TcpTransport,
) -> Result<HandshakeMessage, Box<dyn std::error::Error>> {
    let (header, payload) = transport.recv()?;

    if header.packet_type != PacketType::Handshake {
        return Err(format!(
            "expected Handshake packet, got {:?}",
            header.packet_type
        )
        .into());
    }

    let msg: HandshakeMessage = rkyv::from_bytes::<HandshakeMessage, rkyv::rancor::Error>(&payload)
        .map_err(|e| format!("failed to deserialise HandshakeMessage: {e}"))?;

    Ok(msg)
}

/// Serialise and send a `HandshakeAck`.
fn send_handshake_ack(
    transport: &mut TcpTransport,
    source_path: &str,
    ack: &HandshakeAck,
) -> Result<(), Box<dyn std::error::Error>> {
    let header = PacketHeader {
        dst_path: source_path.to_owned(),
        src_path: "/router".to_owned(),
        packet_type: PacketType::HandshakeAck,
    };
    let payload = rkyv::to_bytes::<rkyv::rancor::Error>(ack)
        .map_err(|e| format!("failed to serialise HandshakeAck: {e}"))?;
    transport.send(&header, &payload)?;
    Ok(())
}

/// Send a `NoBranchError` response back to the sender of a request.
fn send_no_branch_error(
    transport: &mut TcpTransport,
    source_node_id: &str,
    original_header: &PacketHeader,
) {
    // We need the request_id to build the response, but we haven't deserialised
    // the payload. Build a response with request_id = 0 as a best-effort.
    // The operator CLI should handle this gracefully.
    let response = TreeResponse {
        request_id: 0,
        status: ResponseStatus::NoBranchError,
        content_type: content::NONE.to_owned(),
        data: Vec::new(),
    };

    let Ok(payload) = rkyv::to_bytes::<rkyv::rancor::Error>(&response) else {
        return;
    };

    let header = PacketHeader {
        dst_path: original_header.src_path.clone(),
        src_path: "/router".to_owned(),
        packet_type: PacketType::Response,
    };

    if let Err(e) = transport.send(&header, &payload) {
        eprintln!("[router] failed to send NoBranchError to {source_node_id}: {e}");
    }
}

/// Re-encode a decoded packet into raw framed bytes for forwarding.
///
/// This rebuilds the frame so the write-thread can send it verbatim.
fn encode_raw_packet(header: &PacketHeader, payload: &[u8]) -> Option<Vec<u8>> {
    let header_bytes = unshell::transport::encode_header(header)?;
    let header_len = header_bytes.len() as u32;
    let payload_len = payload.len() as u32;

    let mut frame = Vec::with_capacity(8 + header_bytes.len() + payload.len());
    frame.extend_from_slice(&header_len.to_be_bytes());
    frame.extend_from_slice(&header_bytes);
    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.extend_from_slice(payload);
    Some(frame)
}
