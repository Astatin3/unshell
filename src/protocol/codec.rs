//! Framed packet encoding and decoding.

use alloc::{boxed::Box, vec::Vec};
use core::fmt;
use rkyv::{Serialize, access, deserialize, rancor::Error, to_bytes, util::AlignedVec};

use crate::protocol::types::{
    ArchivedCallMessage, ArchivedDataMessage, ArchivedFaultMessage, ArchivedPacketHeader,
};
use crate::protocol::{CallMessage, DataMessage, FaultMessage, PacketHeader, PacketType};

/// Owned framed packet bytes.
pub type FrameBytes = Box<[u8]>;

/// Framing or archive failure.
#[derive(Debug)]
pub enum FrameError {
    /// The frame is truncated or contains trailing bytes.
    Truncated,
    /// Header bytes were not a valid archive.
    InvalidHeader(Error),
    /// Payload bytes were not a valid archive.
    InvalidPayload(Error),
    /// Serialization failed.
    Serialize(Error),
    /// The framed section exceeded the `u32` wire limit.
    LengthOverflow,
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated frame"),
            Self::InvalidHeader(error) => write!(f, "invalid archived header: {error}"),
            Self::InvalidPayload(error) => write!(f, "invalid archived payload: {error}"),
            Self::Serialize(error) => write!(f, "serialization failed: {error}"),
            Self::LengthOverflow => f.write_str("framed section exceeds u32 length"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FrameError {}

/// Borrowed view over a framed packet.
pub struct ParsedFrame<'a> {
    header: PacketHeader,
    payload_bytes: &'a [u8],
}

impl<'a> ParsedFrame<'a> {
    /// Returns the decoded header.
    pub fn header(&self) -> &PacketHeader {
        &self.header
    }

    /// Returns the packet type.
    pub fn packet_type(&self) -> PacketType {
        self.header.packet_type
    }

    /// Returns the raw payload byte section.
    pub fn payload_bytes(&self) -> &'a [u8] {
        self.payload_bytes
    }

    /// Returns an owned header copy.
    pub fn deserialize_header(&self) -> PacketHeader {
        self.header.clone()
    }

    /// Decodes the payload as a call.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] when the payload bytes are not a valid archived call.
    pub fn deserialize_call(&self) -> Result<CallMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedCallMessage, CallMessage>(self.payload_bytes)
    }

    /// Decodes the payload as data.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] when the payload bytes are not a valid archived data packet.
    pub fn deserialize_data(&self) -> Result<DataMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedDataMessage, DataMessage>(self.payload_bytes)
    }

    /// Decodes the payload as a fault.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError`] when the payload bytes are not a valid archived fault.
    pub fn deserialize_fault(&self) -> Result<FaultMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedFaultMessage, FaultMessage>(self.payload_bytes)
    }
}

/// Encodes a packet header and payload into the canonical framed representation.
///
/// # Errors
///
/// Returns [`FrameError`] when serialization fails or a framed section exceeds the wire limit.
pub fn encode_packet<P>(header: &PacketHeader, payload: &P) -> Result<FrameBytes, FrameError>
where
    P: for<'a> Serialize<
        rkyv::api::high::HighSerializer<AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, Error>,
    >,
{
    // WARNING: the simulated and TCP transports both move complete framed packets.
    // One owned contiguous buffer at this boundary is therefore intentional and avoids
    // scattering later hidden copies through routing code.
    let header_bytes = to_bytes::<Error>(header).map_err(FrameError::Serialize)?;
    let payload_bytes = to_bytes::<Error>(payload).map_err(FrameError::Serialize)?;
    let header_len = u32::try_from(header_bytes.len()).map_err(|_| FrameError::LengthOverflow)?;
    let payload_len = u32::try_from(payload_bytes.len()).map_err(|_| FrameError::LengthOverflow)?;

    let mut frame = Vec::with_capacity(8 + header_bytes.len() + payload_bytes.len());
    frame.extend_from_slice(&header_len.to_be_bytes());
    frame.extend_from_slice(&header_bytes);
    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.extend_from_slice(&payload_bytes);
    Ok(frame.into_boxed_slice())
}

/// Decodes a framed packet into a borrowed parsed view.
///
/// # Errors
///
/// Returns [`FrameError`] when the frame is truncated or the header archive is invalid.
pub fn decode_frame(bytes: &[u8]) -> Result<ParsedFrame<'_>, FrameError> {
    if bytes.len() < 8 {
        return Err(FrameError::Truncated);
    }

    let header_len = u32::from_be_bytes(
        bytes
            .get(0..4)
            .ok_or(FrameError::Truncated)?
            .try_into()
            .expect("slice width checked"),
    ) as usize;
    let header_start = 4usize;
    let header_end = header_start + header_len;
    if header_end + 4 > bytes.len() {
        return Err(FrameError::Truncated);
    }

    let payload_len = u32::from_be_bytes(
        bytes
            .get(header_end..header_end + 4)
            .ok_or(FrameError::Truncated)?
            .try_into()
            .expect("slice width checked"),
    ) as usize;
    let payload_start = header_end + 4;
    let payload_end = payload_start + payload_len;
    if payload_end != bytes.len() {
        return Err(FrameError::Truncated);
    }

    // WARNING: the wire format puts a 4-byte length prefix before each archived section.
    // That means the section start is not guaranteed to satisfy rkyv's aligned-access
    // requirements. The header is copied into one temporary `AlignedVec` here because
    // routing cannot proceed safely without a validated header.
    let aligned_header = align_section(
        bytes
            .get(header_start..header_end)
            .ok_or(FrameError::Truncated)?,
    );
    let archived_header = access::<ArchivedPacketHeader, Error>(&aligned_header)
        .map_err(FrameError::InvalidHeader)?;
    let header =
        deserialize::<PacketHeader, Error>(archived_header).map_err(FrameError::InvalidHeader)?;

    Ok(ParsedFrame {
        header,
        payload_bytes: bytes
            .get(payload_start..payload_end)
            .ok_or(FrameError::Truncated)?,
    })
}

/// Deserializes a standalone archived byte section.
///
/// # Errors
///
/// Returns [`FrameError`] when the archived bytes are invalid for the requested type.
pub fn deserialize_archived_bytes<A, T>(bytes: &[u8]) -> Result<T, FrameError>
where
    A: rkyv::Portable
        + for<'b> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'b, Error>>,
    T: rkyv::Archive,
    A: rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<Error>>,
{
    let aligned = align_section(bytes);
    let archived = access::<A, Error>(&aligned).map_err(FrameError::InvalidPayload)?;
    deserialize::<T, Error>(archived).map_err(FrameError::InvalidPayload)
}

fn align_section(bytes: &[u8]) -> AlignedVec {
    let mut aligned = AlignedVec::with_capacity(bytes.len());
    aligned.extend_from_slice(bytes);
    aligned
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{HookTarget, PacketType};
    use alloc::{string::String, vec};

    #[test]
    fn framing_roundtrip_preserves_call() {
        let header = PacketHeader {
            packet_type: PacketType::Call,
            src_path: Vec::new(),
            dst_path: vec![String::from("child")],
            dst_leaf: Some(String::from("echo")),
            hook_id: None,
        };
        let call = CallMessage {
            procedure_id: String::from("org.product.v1.echo.roundtrip"),
            data: b"ping".to_vec(),
            response_hook: Some(HookTarget {
                hook_id: 1,
                return_path: Vec::new(),
            }),
        };

        let frame = encode_packet(&header, &call).expect("frame should encode");
        let parsed = decode_frame(&frame).expect("frame should decode");
        assert_eq!(parsed.deserialize_header(), header);
        assert_eq!(parsed.deserialize_call().expect("call should decode"), call);
    }
}
