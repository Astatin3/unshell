//! # Protocol Module
//!
//! All wire types used by the UnShell protocol.
//!
//! ## Module layout
//!
//! ```text
//! protocol/
//!   mod.rs      ← you are here; re-exports everything
//!   types.rs    ← PacketHeader, TreeRequest, TreeResponse, Handshake*
//!   content.rs  ← content-type string constants
//! ```
//!
//! ## Quick start
//!
//! ```rust
//! use unshell::protocol::{
//!     PacketHeader, PacketType,
//!     TreeRequest, RequestType,
//!     content,
//! };
//!
//! let header = PacketHeader {
//!     dst_path: "/agents/abc123/shell/exec".into(),
//!     src_path: "/operator/sess1".into(),
//!     packet_type: PacketType::Request,
//! };
//!
//! let request = TreeRequest {
//!     request_id: 1,
//!     request_type: RequestType::CallProcedure,
//!     content_type: content::UTF8_STRING.into(),
//!     data: b"ls -la".to_vec(),
//! };
//! ```

pub mod content;
mod types;

pub use types::*;
