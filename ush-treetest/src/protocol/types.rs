//! # Protocol Types
//!
//! This module defines the core types for the UnShell protocol.
//! Uses rkyv for zero-copy serialization.
//!
//! # Serialization
//!
//! All types implement `rkyv::Archive`, `rkyv::Serialize`, and `rkyv::Deserialize`
//! for efficient serialization without runtime type information.
//!
//! # Example
//!
//! ```no_run
//! use ush_treetest::protocol::{TreeRequest, TreeResponse};
//!
//! // Serialize a request
//! let request = TreeRequest::Exec { cmd: "echo hello".to_string() };
//! let bytes = request.to_bytes();
//!
//! // Deserialize it back
//! let decoded = TreeRequest::from_bytes(&bytes).unwrap();
//! ```

use rkyv::{Archive, Serialize, Deserialize};
use std::string::String;
use std::vec::Vec;

/// Default buffer size for rkyv serialization.
///
/// This value is chosen to accommodate typical protocol messages.
const BUFFER_SIZE: usize = 4096;

/// Frame type enum - distinguishes between different frame kinds.
///
/// Each frame type has a specific purpose in the protocol:
/// - `Request` / `Response`: Request-response pairs
/// - `StreamOpen` / `StreamData` / `StreamClose`: Streaming operations
/// - `Handshake` / `HandshakeAck`: Connection setup
///
/// # Example
/// ```
/// use ush_treetest::protocol::FrameType;
///
/// let frame_type = FrameType::Request;
/// assert_eq!(frame_type as u8, 0x01);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    /// Request frame - client requesting an operation
    Request = 0x01,
    /// Response frame - server responding to a request
    Response = 0x02,
    /// Stream open frame - requesting a stream
    StreamOpen = 0x03,
    /// Stream data frame - sending data on a stream
    StreamData = 0x04,
    /// Stream close frame - closing a stream
    StreamClose = 0x05,
    /// Handshake frame - connection initialization
    Handshake = 0x10,
    /// Handshake acknowledgement - connection acceptance
    HandshakeAck = 0x11,
}

impl FrameType {
    /// Convert a byte value to a FrameType.
    ///
    /// # Arguments
    /// * `v` - The byte value to convert
    ///
    /// # Returns
    /// Some(FrameType) if valid, None otherwise
    ///
    /// # Example
    /// ```
    /// use ush_treetest::protocol::FrameType;
    ///
    /// let ft = FrameType::from_u8(0x01);
    /// assert_eq!(ft, Some(FrameType::Request));
    ///
    /// let invalid = FrameType::from_u8(0xFF);
    /// assert_eq!(invalid, None);
    /// ```
    #[allow(dead_code)]
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Request),
            0x02 => Some(Self::Response),
            0x03 => Some(Self::StreamOpen),
            0x04 => Some(Self::StreamData),
            0x05 => Some(Self::StreamClose),
            0x10 => Some(Self::Handshake),
            0x11 => Some(Self::HandshakeAck),
            _ => None,
        }
    }
}

/// Frame header - the metadata sent before each payload.
///
/// The header contains routing information and identifies the frame type.
///
/// # Fields
/// * `frame_type` - The type of frame
/// * `dst_path` - Optional destination path for routing
/// * `src_path` - Source path for the frame
/// * `request_id` - Optional request ID for correlation
/// * `stream_id` - Optional stream ID for streaming
///
/// # Example
/// ```
/// use ush_treetest::protocol::{FrameHeader, FrameType};
///
/// let header = FrameHeader {
///     frame_type: FrameType::Request,
///     dst_path: Some("/shell".to_string()),
///     src_path: "/client".to_string(),
///     request_id: Some(1),
///     stream_id: None,
/// };
///
/// // Serialize and deserialize
/// let bytes = header.to_bytes();
/// let decoded = FrameHeader::from_bytes(&bytes).unwrap();
/// assert_eq!(decoded.frame_type, FrameType::Request);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct FrameHeader {
    /// The type of this frame
    pub frame_type: FrameType,
    /// Destination path for routing (None for responses)
    pub dst_path: Option<String>,
    /// Source path of the sender
    pub src_path: String,
    /// Request ID for correlation (for request/response)
    pub request_id: Option<u64>,
    /// Stream ID (for stream operations)
    pub stream_id: Option<u16>,
}

impl FrameHeader {
    /// Serialize the header to bytes.
    ///
    /// # Returns
    /// Serialized bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<FrameHeader, BUFFER_SIZE>(self)
            .unwrap()
            .into_vec()
    }

    /// Deserialize header from bytes.
    ///
    /// # Arguments
    /// * `bytes` - Serialized bytes
    ///
    /// # Returns
    /// Deserialized header
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Tree request - operations on the tree.
///
/// These requests are sent from clients to servers to perform operations.
///
/// # Example
/// ```
/// use ush_treetest::protocol::TreeRequest;
///
/// // Execute a command
/// let request = TreeRequest::Exec { cmd: "echo hello".to_string() };
/// let bytes = request.to_bytes();
/// let decoded = TreeRequest::from_bytes(&bytes).unwrap();
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum TreeRequest {
    /// List child nodes at a path
    ListNodes {},
    /// List endpoints at a path
    ListEndpoints {},
    /// List all leaf paths in the tree
    ListLeaves {},
    /// Get information about a node
    GetInfo { path: String },
    /// Execute a command
    Exec { cmd: String },
    /// Open a stream to a path
    StreamOpen { path: String },
    /// Resize a terminal
    Resize { rows: u16, cols: u16 },
}

impl TreeRequest {
    /// Serialize the request to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<TreeRequest, BUFFER_SIZE>(self)
            .unwrap()
            .into_vec()
    }

    /// Deserialize request from bytes.
    ///
    /// # Arguments
    /// * `bytes` - Serialized bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Tree response - results from tree operations.
///
/// These responses are sent from servers to clients.
///
/// # Example
/// ```
/// use ush_treetest::protocol::TreeResponse;
///
/// let response = TreeResponse::ExecOutput {
///     exit_code: 0,
///     stdout: b"hello".to_vec(),
///     stderr: b"".to_vec(),
/// };
/// let bytes = response.to_bytes();
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub enum TreeResponse {
    /// List of child node names
    NodeList { names: Vec<String> },
    /// List of endpoints
    EndpointList { endpoints: Vec<EndpointInfo> },
    /// List of leaf paths
    LeafList { leaves: Vec<String> },
    /// Node information
    NodeInfo { info: NodeInfo },
    /// Command execution output
    ExecOutput {
        exit_code: i32,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
    },
    /// Stream opened confirmation
    StreamOpened { stream_id: u16 },
}

impl TreeResponse {
    /// Serialize the response to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<TreeResponse, BUFFER_SIZE>(self)
            .unwrap()
            .into_vec()
    }

    /// Deserialize response from bytes.
    ///
    /// # Arguments
    /// * `bytes` - Serialized bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Information about an endpoint.
///
/// # Fields
/// * `name` - The endpoint name
/// * `path` - The path where the endpoint is registered
/// * `endpoint_type` - The type of endpoint
///
/// # Example
/// ```
/// use ush_treetest::protocol::{EndpointInfo, EndpointType};
///
/// let info = EndpointInfo {
///     name: "shell".to_string(),
///     path: "/shell".to_string(),
///     endpoint_type: EndpointType::Leaf,
/// };
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct EndpointInfo {
    /// The endpoint name
    pub name: String,
    /// The path where this endpoint is registered
    pub path: String,
    /// The type of this endpoint
    pub endpoint_type: EndpointType,
}

/// Type of endpoint.
///
/// # Example
/// ```
/// use ush_treetest::protocol::EndpointType;
///
/// let leaf_type = EndpointType::Leaf;
/// assert!(matches!(leaf_type, EndpointType::Leaf));
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone, Copy)]
#[repr(u8)]
pub enum EndpointType {
    /// Leaf endpoint - executes commands
    Leaf = 0x01,
    /// Proxy endpoint - routes to other endpoints
    Proxy = 0x02,
    /// Stream endpoint - provides streaming
    Stream = 0x03,
}

/// Information about a node in the tree.
///
/// # Fields
/// * `path` - The node path
/// * `is_leaf` - Whether this is a leaf node
/// * `has_children` - Whether this node has children
/// * `endpoints` - List of endpoint names at this node
///
/// # Example
/// ```
/// use ush_treetest::protocol::NodeInfo;
///
/// let info = NodeInfo {
///     path: "/shell".to_string(),
///     is_leaf: true,
///     has_children: false,
///     endpoints: vec!["shell".to_string()],
/// };
/// assert!(info.is_leaf);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct NodeInfo {
    /// The node path
    pub path: String,
    /// Whether this is a leaf node (endpoint with no children)
    pub is_leaf: bool,
    /// Whether this node has children
    pub has_children: bool,
    /// Names of endpoints at this node
    pub endpoints: Vec<String>,
}

/// Handshake message - sent when connecting.
///
/// The client sends registered paths during handshake.
///
/// # Fields
/// * `registered_paths` - Paths the client wants to register
///
/// # Example
/// ```
/// use ush_treetest::protocol::Handshake;
///
/// let handshake = Handshake {
///     registered_paths: vec!["/client".to_string()],
/// };
/// let bytes = handshake.to_bytes();
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct Handshake {
    /// Paths the client wants to register
    pub registered_paths: Vec<String>,
}

impl Handshake {
    /// Serialize the handshake to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<Handshake, BUFFER_SIZE>(self)
            .unwrap()
            .into_vec()
    }

    /// Deserialize handshake from bytes.
    #[allow(dead_code)]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}

/// Handshake acknowledgement - router's response to handshake.
///
/// # Fields
/// * `accepted` - Whether the handshake was accepted
/// * `assigned_base_path` - Base path assigned by the server
///
/// # Example
/// ```
/// use ush_treetest::protocol::HandshakeAck;
///
/// let ack = HandshakeAck {
///     accepted: true,
///     assigned_base_path: "/client".to_string(),
/// };
/// assert!(ack.accepted);
/// ```
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
pub struct HandshakeAck {
    /// Whether the handshake was accepted
    pub accepted: bool,
    /// Base path assigned by the server
    pub assigned_base_path: String,
}

impl HandshakeAck {
    /// Serialize the acknowledgement to bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        rkyv::to_bytes::<HandshakeAck, BUFFER_SIZE>(self)
            .unwrap()
            .into_vec()
    }

    /// Deserialize acknowledgement from bytes.
    ///
    /// # Arguments
    /// * `bytes` - Serialized bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        unsafe { rkyv::from_bytes_unchecked(bytes) }.map_err(|e| e.to_string())
    }
}