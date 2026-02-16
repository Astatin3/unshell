//! Protocol stacking system for extensible network communication.
//!
//! This module provides a way to layer multiple protocols on top of each other,
//! similar to a network stack. Each protocol can encode/decode data from the layer below.
//!
//! # Architecture
//!
//! Each protocol implements the `Protocol` trait, defining:
//! - How to encode data going "out" (to the network)
//! - How to decode data coming "in" (from the network)
//! - Configuration for the protocol
//!
//! # Usage
//!
//! ```rust
//! use tree::protocols::{Protocol, ProtocolStack, ProtocolConfig};
//! use serde_json::json;
//!
//! // Create a stack: base64 -> http -> tcp
//! let stack: ProtocolStack = vec![
//!     ProtocolConfig::Base64(json!({})),
//!     ProtocolConfig::Http(json!({
//!         "method": "POST",
//!         "path": "/api/data"
//!     })),
//! ];
//!
//! // Encode outgoing message
//! let encoded = stack.encode(&json!({"action": "test"}))?;
//!
//! // Decode incoming data
//! let decoded = stack.decode(&encoded)?;
//! ```

pub mod base64;
pub mod http;
pub mod stack;

pub use stack::{
    Base64Config, HttpConfig, Protocol, ProtocolConfig, ProtocolError, ProtocolStack, TcpConfig,
    WebSocketConfig,
};
