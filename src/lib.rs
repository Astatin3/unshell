//! UnShell core protocol crate.
//!
//! The crate now models the draft protocol in `PROTOCOL.md` directly:
//!
//! - [`protocol`] provides the canonical wire types, framing helpers, validation,
//!   and introspection payloads.
//! - [`tree`] provides an explicit enum-based tree declaration, longest-prefix
//!   routing helpers, and a small endpoint runtime for tests.
//! - [`transport`] provides framed transport implementations for simulated
//!   channel-based links and TCP links.
//! - [`logger`] remains available for lightweight logging.
//!
//! ```rust
//! use unshell::protocol::{CallMessage, HookTarget, PacketHeader, PacketType, encode_packet};
//!
//! let header = PacketHeader {
//!     packet_type: PacketType::Call,
//!     src_path: Vec::new(),
//!     dst_path: vec!["child".into()],
//!     dst_leaf: Some("echo".into()),
//!     hook_id: None,
//! };
//! let call = CallMessage {
//!     procedure_id: "org.product.v1.echo.roundtrip".into(),
//!     data: b"ping".to_vec(),
//!     response_hook: Some(HookTarget {
//!         hook_id: 1,
//!         return_path: Vec::new(),
//!     }),
//! };
//!
//! let frame = encode_packet(&header, &call).expect("call should encode");
//! assert!(!frame.is_empty());
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
extern crate alloc;

pub mod logger;
pub mod protocol;
pub mod transport;
pub mod tree;

pub use ush_obfuscate as obfuscate;
