//! # Router Core
//!
//! The main accept loop. Binds a TCP listener and spawns a node thread for
//! each incoming connection.

use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use crate::registry::NodeRegistry;
use crate::node::spawn_node;

/// Start the router, binding to `bind_addr` and accepting connections forever.
///
/// This function blocks until an unrecoverable error occurs.
///
/// # Errors
///
/// Returns an error if the bind fails (e.g., port already in use).
///
/// # Example
///
/// ```rust,no_run
/// ush_router::router::run("0.0.0.0:9000").expect("router failed");
/// ```
pub fn run(bind_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(bind_addr)?;
    eprintln!("[router] listening on {bind_addr}");

    let registry = Arc::new(Mutex::new(NodeRegistry::new()));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let addr = stream
                    .peer_addr()
                    .map(|a| a.to_string())
                    .unwrap_or_else(|_| "unknown".into());
                eprintln!("[router] new connection from {addr}");
                spawn_node(stream, Arc::clone(&registry));
            }
            Err(e) => {
                eprintln!("[router] accept error: {e}");
                // Non-fatal; keep accepting.
            }
        }
    }

    Ok(())
}
