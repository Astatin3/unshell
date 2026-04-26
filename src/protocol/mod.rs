//! Canonical UnShell protocol surface.
//!
//! This module is the stable facade for wire-level protocol types, framing, and
//! stateless validation helpers. Callers normally:
//! - build one [`PacketHeader`] plus payload type from this module,
//! - encode it with [`encode_packet`],
//! - decode inbound bytes with [`decode_frame`], and
//! - validate message/header shape with [`validate_header`], [`validate_call`], and
//!   [`validate_procedure_id`].
//!
//! The concrete wire structs live in the private `types` module and are re-exported here so the
//! public API stays flat while internal archived-type details remain hidden.
//!
//! # Example
//! ```rust
//! use unshell::protocol::{
//!     CallMessage, PacketHeader, PacketType, decode_frame, encode_packet, validate_call,
//!     validate_header,
//! };
//!
//! let header = PacketHeader {
//!     packet_type: PacketType::Call,
//!     src_path: vec!["root".into()],
//!     dst_path: vec!["root".into(), "worker".into()],
//!     dst_leaf: Some("service".into()),
//!     hook_id: None,
//! };
//! let call = CallMessage {
//!     procedure_id: "example.service.v1.invoke".into(),
//!     data: vec![1, 2, 3],
//!     response_hook: None,
//! };
//!
//! validate_header(&header).unwrap();
//! validate_call(&header, &call).unwrap();
//! let frame = encode_packet(&header, &call)?;
//! let parsed = decode_frame(&frame)?;
//! let decoded = parsed.deserialize_call()?;
//! assert_eq!(decoded.procedure_id, call.procedure_id);
//! # Ok::<(), unshell::protocol::FrameError>(())
//! ```

pub mod codec;
pub mod introspection;
pub mod tree;
mod types;
pub mod validation;

#[cfg(test)]
mod tests;

pub use codec::{
    FrameBytes, FrameError, ParsedFrame, SECTION_ALIGN, decode_frame, deserialize_archived_bytes,
    encode_packet,
};
pub use introspection::{
    EndpointIntrospection, INTROSPECTION_PROCEDURE_ID, LeafIntrospection, LeafIntrospectionSummary,
};
pub use types::{
    CallMessage, DataMessage, FaultMessage, HookTarget, PacketHeader, PacketType, ProtocolFault,
};
pub use validation::{ValidationError, validate_call, validate_header, validate_procedure_id};
