//! Canonical UnShell protocol modules.
//!
//! The wire model matches `PROTOCOL.md` directly.

pub mod codec;
pub mod introspection;
mod types;
pub mod validation;

pub use codec::{
    FrameBytes, FrameError, ParsedFrame, decode_frame, deserialize_archived_bytes, encode_packet,
};
pub use introspection::{EndpointIntrospection, LeafIntrospection, LeafIntrospectionSummary};
pub use types::{
    CallMessage, DataMessage, FaultMessage, HookTarget, PacketHeader, PacketType, ProtocolFault,
};
pub use validation::{ValidationError, validate_call, validate_header, validate_procedure_id};
