//! Canonical UnShell protocol modules.

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
    EndpointIntrospection, INTROSPECTION_PROCEDURE_ID, LeafIntrospection,
    LeafIntrospectionSummary,
};
pub use types::{
    CallMessage, DataMessage, FaultMessage, HookTarget, PacketHeader, PacketType, ProtocolFault,
};
pub use validation::{ValidationError, validate_call, validate_header, validate_procedure_id};
