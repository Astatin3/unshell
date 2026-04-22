//! # Server Implementation
//!
//! This module provides the server functionality for handling incoming connections.

use crate::protocol::{
    FrameHeader, FrameType, Handshake, TreeRequest, TreeResponse, TcpTransport, Transport,
    make_response, make_handshake_ack,
};
use crate::tree::Tree;
use crate::leaves::{ProxyEndpoint, RemoteShell, TTY};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};

/// Global client counter for assigning unique base paths.
static CLIENT_COUNT: AtomicU32 = AtomicU32::new(0);

/// Default listening address for the server.
///
/// # Example
/// ```
/// let addr = ush_treetest::server::DEFAULT_ADDR;
/// assert_eq!(addr, "0.0.0.0:8080");
/// ```
#[allow(dead_code)]
pub const DEFAULT_ADDR: &str = "0.0.0.0:8080";

/// Run the server with the given address.
///
/// This function starts listening on the specified address and handles incoming
/// connections in separate threads.
///
/// # Arguments
/// * `addr` - The address to listen on (e.g., "0.0.0.0:8080")
///
/// # Example
/// ```
/// run_server("0.0.0.0:8080");
/// ```
pub fn run_server(addr: &str) -> ! {
    log::info!("Starting server on {}", addr);

    let tree = Arc::new(Mutex::new(Tree::new()));
    {
        let mut tree = tree.lock().unwrap();
        tree.add_endpoint("/", Box::new(ProxyEndpoint::new_empty("proxy")));
        tree.add_endpoint("/shell", Box::new(RemoteShell::new("shell")));
        tree.add_endpoint("/tty", Box::new(TTY::new("tty")));
    }

    let listener = TcpTransport::listen(addr).expect("failed to bind");
    log::info!("Listening on {}", addr);

    loop {
        match TcpTransport::accept(&listener) {
            Ok(transport) => {
                log::info!("New connection from {:?}", transport.peer_addr());
                let tree = Arc::clone(&tree);
                std::thread::spawn(move || {
                    handle_connection(transport, tree);
                });
            }
            Err(e) => {
                log::error!("accept error: {:?}", e);
            }
        }
    }
}

/// Handle a single connection.
///
/// This function handles the handshake and then processes frames in a loop until
/// the connection is closed.
///
/// # Arguments
/// * `transport` - The TCP transport for the connection
/// * `tree` - Shared access to the tree
pub fn handle_connection(mut transport: TcpTransport, tree: Arc<Mutex<Tree>>) {
    let (header, payload) = match transport.recv_frame() {
        Ok(h) => h,
        Err(e) => {
            log::error!("recv error: {:?}", e);
            return;
        }
    };

    if header.frame_type != FrameType::Handshake {
        log::error!("expected handshake");
        return;
    }

    log::info!("Client connected");

    let base_path = if payload.is_empty() {
        let client_num = CLIENT_COUNT.fetch_add(1, Ordering::SeqCst);
        format!("/client_{}", client_num)
    } else {
        let handshake = match Handshake::from_bytes(&payload) {
            Ok(h) => h,
            Err(e) => {
                log::error!("handshake parse error: {}", e);
                return;
            }
        };
        let client_num = CLIENT_COUNT.fetch_add(1, Ordering::SeqCst);
        if handshake.registered_paths.is_empty() {
            format!("/client_{}", client_num)
        } else {
            handshake.registered_paths.first().cloned().unwrap_or_else(|| {
                format!("/client_{}", client_num)
            })
        }
    };

    let (ack_header, ack_payload) = make_handshake_ack(true, &base_path);
    transport.send_frame(&ack_header, Some(&ack_payload)).expect("send failed");

    loop {
        match transport.recv_frame() {
            Ok((header, payload)) => {
                let response = handle_frame(&header, &payload, &tree);

                if let Some(response) = response {
                    let (resp_header, resp_payload) = match response {
                        Ok((h, p)) => (h, p),
                        Err(e) => {
                            log::error!("handle error: {:?}", e);
                            break;
                        }
                    };
                    transport.send_frame(&resp_header, Some(&resp_payload)).expect("send failed");
                }

                if header.frame_type == FrameType::StreamClose {
                    break;
                }
            }
            Err(e) => {
                log::error!("recv error: {:?}", e);
                break;
            }
        }
    }

    log::info!("Connection closed");
}

/// Handle a single frame and return an optional response.
///
/// # Arguments
/// * `header` - The frame header
/// * `payload` - The frame payload bytes
/// * `tree` - Shared access to the tree
///
/// # Returns
/// * `Some(Ok((header, payload)))` for a response to send
/// * `Some(Err(e))` for an error
/// * `None` for no response (async handling)
///
/// # Example
/// ```
/// use ush_treetest::protocol::{FrameType, FrameHeader, TcpTransport};
///
/// let header = FrameHeader {
///     frame_type: FrameType::Request,
///     dst_path: Some("/shell".to_string()),
///     src_path: "/client".to_string(),
///     request_id: Some(1),
///     stream_id: None,
/// };
/// let payload = vec![];
///
/// if let Some(result) = handle_frame(&header, &payload, &tree) {
///     // Handle response
/// }
/// ```
pub fn handle_frame(
    header: &FrameHeader,
    payload: &[u8],
    tree: &Arc<Mutex<Tree>>,
) -> Option<Result<(FrameHeader, Vec<u8>), String>> {
    match header.frame_type {
        FrameType::Request => {
            let request: TreeRequest = match TreeRequest::from_bytes(payload) {
                Ok(r) => r,
                Err(e) => return Some(Err(e.to_string())),
            };

            let dst_path = header.dst_path.as_deref().unwrap_or("/");

            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };

            let response = match request {
                TreeRequest::ListNodes {} => {
                    let names = tree.list_nodes_at(dst_path);
                    TreeResponse::NodeList { names }
                }
                TreeRequest::ListEndpoints {} => {
                    let endpoints = tree.list_endpoints_at(dst_path);
                    TreeResponse::EndpointList { endpoints }
                }
                TreeRequest::ListLeaves {} => {
                    let leaves = tree.list_leaves();
                    TreeResponse::LeafList { leaves }
                }
                TreeRequest::GetInfo { path } => {
                    match tree.get_info(&path) {
                        Ok(info) => TreeResponse::NodeInfo { info },
                        Err(e) => return Some(Err(e)),
                    }
                }
                TreeRequest::Exec { ref cmd } => {
                    let (handler, matched_path) = match tree.find_handler(dst_path) {
                        Some(h) => h,
                        None => return Some(Err(format!("path not found: {}", dst_path))),
                    };
                    let result = {
                        let mut handler = match handler.lock() {
                            Ok(h) => h,
                            Err(e) => return Some(Err(format!("lock error: {}", e))),
                        };
                        handler.handle_request(&TreeRequest::Exec { cmd: cmd.clone() }, matched_path)
                    };
                    match result {
                        Ok(resp) => resp,
                        Err(e) => return Some(Err(e)),
                    }
                }
                TreeRequest::StreamOpen { path } => {
                    match tree.open_stream(&path, &header.src_path) {
                        Ok(stream_id) => TreeResponse::StreamOpened { stream_id },
                        Err(e) => return Some(Err(e)),
                    }
                }
                TreeRequest::Resize { .. } => {
                    return Some(Err("unsupported request: Resize".to_string()));
                }
            };

            Some(Ok(make_response(
                &header.src_path,
                header.request_id.unwrap_or(0),
                &response,
            )))
        }

        FrameType::StreamOpen => {
            let dst_path = header.dst_path.as_deref().unwrap_or("/");
            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };
            match tree.open_stream(dst_path, &header.src_path) {
                Ok(stream_id) => {
                    let response = TreeResponse::StreamOpened { stream_id };
                    Some(Ok(make_response(
                        &header.src_path,
                        header.request_id.unwrap_or(0),
                        &response,
                    )))
                }
                Err(e) => Some(Err(e)),
            }
        }

        FrameType::StreamData => {
            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };
            tree.route_stream_data(header, payload).ok();
            None
        }

        FrameType::StreamClose => {
            let mut tree = match tree.lock() {
                Ok(t) => t,
                Err(e) => return Some(Err(format!("lock error: {}", e))),
            };
            if let Some(stream_id) = header.stream_id {
                tree.close_stream(stream_id).ok();
            }
            None
        }

        _ => Some(Err("unsupported frame type".to_string())),
    }
}