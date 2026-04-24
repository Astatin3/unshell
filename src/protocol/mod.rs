//! Canonical UnShell protocol modules.
//!
//! The wire model matches `PROTOCOL.md` directly.

pub mod codec;
pub mod introspection;
pub mod traits;
pub mod tree;
mod types;
pub mod validation;

#[cfg(test)]
mod tests;

pub use codec::{
    FrameBytes, FrameCodec, FrameError, ParsedFrame, RkyvCodec, deserialize_archived_bytes,
};
pub use introspection::{EndpointIntrospection, LeafIntrospection, LeafIntrospectionSummary};
pub use traits::{HookStore, LeafMetadata, PacketFraming, PacketProcessor, RouteResolution};
pub use types::{
    CallMessage, DataMessage, FaultMessage, HookTarget, PacketHeader, PacketType, ProtocolFault,
};
pub use validation::{ValidationError, validate_call, validate_header, validate_procedure_id};

pub fn encode_packet<P>(header: &PacketHeader, payload: &P) -> Result<FrameBytes, FrameError>
where
    P: for<'a> rkyv::Serialize<
            rkyv::api::high::HighSerializer<
                rkyv::util::AlignedVec,
                rkyv::ser::allocator::ArenaHandle<'a>,
                rkyv::rancor::Error,
            >,
        >,
{
    codec::encode_packet(header, payload)
}

pub fn decode_frame(bytes: &[u8]) -> Result<ParsedFrame<'_>, FrameError> {
    codec::decode_frame(bytes)
}
