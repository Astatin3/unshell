//! # Client Implementation
//!
//! This module provides the client functionality for connecting to servers,
//! sending requests, and managing streams.

use crate::protocol::{
    FrameType, TreeRequest, TreeResponse, TcpTransport, Transport,
    make_request, make_stream_open, make_stream_data, make_stream_close,
    make_handshake,
};
use crate::tree::Tree;
use crate::leaves::{RemoteShell, TTY};
use std::string::String;
use std::vec::Vec;
use std::fmt;

/// Client state - manages connection and local tree.
///
/// # Example
/// ```
/// use ush_treetest::client::Client;
///
/// // Start with local tree
/// let mut client = Client::new_local();
/// println!("Leaves: {:?}", client.list_leaves());
///
/// // Connect to remote server
/// client.connect("localhost:8080").unwrap();
/// ```
///
/// # Fields
/// * `transport` - Optional TCP transport for remote connection
/// * `tree` - Local tree for local operations
/// * `current_path` - Current working path
/// * `request_id` - Next request ID to send
/// * `stream_id` - Next stream ID to allocate
/// * `streams` - Active streams
/// * `base_path` - Base path assigned by server
/// * `mode` - Operation mode (Local or Connected)
#[allow(dead_code)]
pub struct Client {
    transport: Option<TcpTransport>,
    #[allow(dead_code)]
    tree: Tree,
    current_path: String,
    request_id: u64,
    #[allow(dead_code)]
    stream_id: u16,
    streams: Vec<StreamState>,
    base_path: String,
    mode: ClientMode,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("transport", &self.transport.is_some())
            .field("current_path", &self.current_path)
            .field("mode", &self.mode)
            .finish()
    }
}

/// Client operation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum ClientMode {
    /// Local-only mode (no remote connection)
    Local,
    /// Connected to remote server
    Connected,
}

/// State of an open stream.
///
/// # Fields
/// * `stream_id` - The stream identifier
/// * `path` - The path this stream is connected to
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct StreamState {
    stream_id: u16,
    path: String,
}

#[allow(dead_code)]
impl Client {
    /// Create a new client with a local tree.
    ///
    /// The local tree has `/shell` and `/tty` endpoints registered.
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// let leaves = client.list_leaves();
    /// assert!(leaves.contains(&"/shell".to_string()));
    /// ```
    pub fn new_local() -> Self {
        let mut tree = Tree::new();
        tree.add_endpoint("/shell", Box::new(RemoteShell::new("shell")));
        tree.add_endpoint("/tty", Box::new(TTY::new("tty")));

        Self {
            transport: None,
            tree,
            current_path: String::from("/"),
            request_id: 1,
            stream_id: 1,
            streams: Vec::new(),
            base_path: String::from("/"),
            mode: ClientMode::Local,
        }
    }

    /// Get the next request ID.
    ///
    /// Each request gets a unique incrementing ID.
    fn next_request_id(&mut self) -> u64 {
        let id = self.request_id;
        self.request_id += 1;
        id
    }

    /// Get the next stream ID.
    #[allow(dead_code)]
    fn next_stream_id(&mut self) -> u16 {
        let id = self.stream_id;
        self.stream_id = self.stream_id.wrapping_add(1);
        id
    }

    /// List nodes at a path.
    ///
    /// # Arguments
    /// * `path` - Optional path (defaults to current path)
    ///
    /// # Returns
    /// List of child node names
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// let nodes = client.list_nodes(None).unwrap();
    /// ```
    pub fn list_nodes(&self, path: Option<&str>) -> Result<Vec<String>, String> {
        let path = path.unwrap_or(&self.current_path);
        self.tree.list_nodes(path)
    }

    /// List endpoints at a path.
    ///
    /// # Arguments
    /// * `path` - Optional path (defaults to current path)
    ///
    /// # Returns
    /// List of endpoint information
    pub fn list_endpoints(
        &self,
        path: Option<&str>,
    ) -> Result<Vec<crate::protocol::EndpointInfo>, String> {
        let path = path.unwrap_or(&self.current_path);
        self.tree.list_endpoints(path)
    }

    /// List all leaf paths.
    ///
    /// # Returns
    /// List of leaf node paths
    ///
    /// # Example
    /// ```
    /// let client = Client::new_local();
    /// let leaves = client.list_leaves();
    /// assert!(!leaves.is_empty());
    /// ```
    pub fn list_leaves(&self) -> Vec<String> {
        self.tree.list_leaves()
    }

    /// Get information about a node.
    ///
    /// # Arguments
    /// * `path` - The path to get info about
    ///
    /// # Returns
    /// Node information
    ///
    /// # Example
    /// ```
    /// let client = Client::new_local();
    /// let info = client.get_info("/shell").unwrap();
    /// assert!(info.is_leaf);
    /// ```
    pub fn get_info(&self, path: &str) -> Result<crate::protocol::NodeInfo, String> {
        self.tree.get_info(path)
    }

    /// Execute a command locally on the tree.
    ///
    /// # Arguments
    /// * `path` - The path to execute at
    /// * `cmd` - The command to execute
    ///
    /// # Returns
    /// Execution response with exit code and output
    pub fn exec_local(&mut self, path: &str, cmd: &str) -> Result<TreeResponse, String> {
        let (handler, matched_path) = self
            .tree
            .find_handler(path)
            .ok_or_else(|| format!("path not found: {}", path))?;

        let request = TreeRequest::Exec {
            cmd: cmd.to_string(),
        };

        let mut handler = handler.lock().map_err(|e| e.to_string())?;
        handler.handle_request(&request, matched_path)
    }

    /// Connect to a remote server.
    ///
    /// # Arguments
    /// * `addr` - The server address (e.g., "localhost:8080")
    ///
    /// # Returns
    /// Ok(()) on success
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// client.connect("localhost:8080").unwrap();
    /// ```
    pub fn connect(&mut self, addr: &str) -> Result<(), String> {
        let transport = TcpTransport::connect(addr).map_err(|e| e.to_string())?;
        self.transport = Some(transport);
        self.mode = ClientMode::Connected;
        self.do_handshake()
    }

    /// Perform handshake with remote server.
    fn do_handshake(&mut self) -> Result<(), String> {
        let transport = self.transport.as_mut().ok_or("not connected")?;
        let (header, payload) = make_handshake(vec![self.current_path.clone()]);
        transport
            .send_frame(&header, Some(&payload))
            .map_err(|e| e.to_string())?;
        let (header, payload) = transport.recv_frame().map_err(|e| e.to_string())?;
        if header.frame_type != FrameType::HandshakeAck {
            return Err("unexpected response type".to_string());
        }
        let ack = crate::protocol::HandshakeAck::from_bytes(&payload)
            .map_err(|e| e.to_string())?;
        if !ack.accepted {
            return Err("handshake rejected".to_string());
        }
        self.base_path = ack.assigned_base_path.clone();
        Ok(())
    }

    /// Send a request to the remote server.
    ///
    /// # Arguments
    /// * `dst_path` - The destination path
    /// * `request` - The request to send
    ///
    /// # Returns
    /// The response from the server
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// client.connect("localhost:8080").unwrap();
    ///
    /// let request = TreeRequest::Exec { cmd: "echo hello".to_string() };
    /// let response = client.send_request("/shell", &request).unwrap();
    /// ```
    pub fn send_request(&mut self, dst_path: &str, request: &TreeRequest) -> Result<TreeResponse, String> {
        let request_id = self.next_request_id();

        let transport = self.transport.as_mut().ok_or("not connected")?;

        let full_path = if dst_path.starts_with('/') {
            dst_path.to_string()
        } else {
            format!("{}/{}", self.current_path, dst_path)
        };

        let (header, payload) = make_request(&full_path, &self.base_path, request_id, request);
        transport
            .send_frame(&header, Some(&payload))
            .map_err(|e| e.to_string())?;

        let (header, payload) = transport.recv_frame().map_err(|e| e.to_string())?;
        if header.frame_type != FrameType::Response {
            return Err("unexpected response type".to_string());
        }

        let response = TreeResponse::from_bytes(&payload).map_err(|e| e.to_string())?;
        Ok(response)
    }

    /// Open a stream to a remote path.
    ///
    /// # Arguments
    /// * `dst_path` - The destination path
    ///
    /// # Returns
    /// The stream ID on success
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// client.connect("localhost:8080").unwrap();
    /// let stream_id = client.open_stream("/tty").unwrap();
    /// ```
    pub fn open_stream(&mut self, dst_path: &str) -> Result<u16, String> {
        let request_id = self.next_request_id();

        let transport = self.transport.as_mut().ok_or("not connected")?;

        let full_path = if dst_path.starts_with('/') {
            dst_path.to_string()
        } else {
            format!("{}/{}", self.current_path, dst_path)
        };

        let header = make_stream_open(&full_path, &self.base_path, request_id);
        transport.send_frame(&header, None).map_err(|e| e.to_string())?;

        let (header, payload) = transport.recv_frame().map_err(|e| e.to_string())?;
        if header.frame_type != FrameType::Response {
            return Err("unexpected response type".to_string());
        }

        let response = TreeResponse::from_bytes(&payload).map_err(|e| e.to_string())?;

        match response {
            TreeResponse::StreamOpened { stream_id } => {
                self.streams.push(StreamState {
                    stream_id,
                    path: full_path,
                });
                Ok(stream_id)
            }
            _ => Err("expected StreamOpened".to_string()),
        }
    }

    /// Send data on a stream.
    ///
    /// # Arguments
    /// * `stream_id` - The stream ID
    /// * `data` - The data to send
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// client.connect("localhost:8080").unwrap();
    /// let stream_id = client.open_stream("/tty").unwrap();
    /// client.send_stream_data(stream_id, b"hello").unwrap();
    /// ```
    pub fn send_stream_data(&mut self, stream_id: u16, data: &[u8]) -> Result<(), String> {
        let transport = self.transport.as_mut().ok_or("not connected")?;
        let (header, payload) = make_stream_data(stream_id, data);
        transport
            .send_frame(&header, Some(&payload))
            .map_err(|e| e.to_string())
    }

    /// Close a stream.
    ///
    /// # Arguments
    /// * `stream_id` - The stream ID to close
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// client.connect("localhost:8080").unwrap();
    /// let stream_id = client.open_stream("/tty").unwrap();
    /// client.close_stream(stream_id).unwrap();
    /// ```
    pub fn close_stream(&mut self, stream_id: u16) -> Result<(), String> {
        let transport = self.transport.as_mut().ok_or("not connected")?;
        let header = make_stream_close(stream_id);
        transport
            .send_frame(&header, None)
            .map_err(|e| e.to_string())?;
        self.streams.retain(|s| s.stream_id != stream_id);
        Ok(())
    }

    /// Check if connected to remote.
    ///
    /// # Returns
    /// True if connected to a remote server
    ///
    /// # Example
    /// ```
    /// let client = Client::new_local();
    /// assert!(!client.is_connected());
    /// ```
    pub fn is_connected(&self) -> bool {
        matches!(self.mode, ClientMode::Connected)
    }

    /// Get current path.
    ///
    /// # Returns
    /// The current working path
    ///
    /// # Example
    /// ```
    /// let client = Client::new_local();
    /// assert_eq!(client.current_path(), "/");
    /// ```
    pub fn current_path(&self) -> &str {
        &self.current_path
    }

    /// Set current path.
    ///
    /// # Arguments
    /// * `path` - The new current path
    ///
    /// # Example
    /// ```
    /// let mut client = Client::new_local();
    /// client.set_path("/shell");
    /// assert_eq!(client.current_path(), "/shell");
    /// ```
    pub fn set_path(&mut self, path: &str) {
        self.current_path = path.to_string();
    }
}