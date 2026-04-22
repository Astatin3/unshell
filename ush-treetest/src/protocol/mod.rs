//! # Protocol Module
//!
//! This module defines the protocol types and transport layer for the unshell tree protocol.
//! It provides serialization via rkyv and TCP transport for frame passing.
//!
//! # Frame Format
//!
//! Each frame consists of:
//! - 4-byte header length (little-endian u32)
//! - Serialized header bytes (using rkyv)
//! - 4-byte payload length (little-endian u32)
//! - Payload bytes (optional)
//!
//! # Usage
//!
//! ```no_run
//! use ush_treetest::protocol::{
//!     FrameType, FrameHeader, TreeRequest, TreeResponse,
//!     TcpTransport, Transport,
//! };
//!
//! // Connect to server
//! let mut transport = TcpTransport::connect("localhost:8080").unwrap();
//!
//! // Send a request
//! let header = FrameHeader {
//!     frame_type: FrameType::Request,
//!     dst_path: Some("/shell".to_string()),
//!     src_path: "/client".to_string(),
//!     request_id: Some(1),
//!     stream_id: None,
//! };
//! let payload = TreeRequest::Exec { cmd: "echo hello".to_string() }.to_bytes();
//! transport.send_frame(&header, Some(&payload)).unwrap();
//!
//! // Receive response
//! let (header, payload) = transport.recv_frame().unwrap();
//! let response = TreeResponse::from_bytes(&payload).unwrap();
//! ```

pub mod types;
pub mod transport;

pub use types::*;
pub use transport::*;