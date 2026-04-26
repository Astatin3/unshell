//! Framed packet encoding and decoding.
use core::{fmt, mem};
use rkyv::{
    Serialize, access, api::high::to_bytes_in, deserialize, rancor::Error, util::AlignedVec,
};

use super::types::{
    ArchivedCallMessage, ArchivedDataMessage, ArchivedFaultMessage, ArchivedPacketHeader,
};
use crate::protocol::{CallMessage, DataMessage, FaultMessage, PacketHeader, PacketType};

/// Archived-section alignment guaranteed by the frame format.
pub const SECTION_ALIGN: usize = 16;

/// Owned framed packet bytes.
pub type FrameBytes = AlignedVec<SECTION_ALIGN>;

/// Framing or archive failure.
#[derive(Debug)]
pub enum FrameError {
    /// The byte slice ended before a full frame could be decoded.
    Truncated,
    /// The archived header bytes failed validation or deserialization.
    InvalidHeader(Error),
    /// The archived payload bytes failed validation or deserialization.
    InvalidPayload(Error),
    /// Serializing one header or payload section failed.
    Serialize(Error),
    /// One archived section grew beyond the `u32` length prefix supported by the format.
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

impl core::error::Error for FrameError {}

/// Parsed frame with one owned header and a borrowed payload section.
///
/// The frame decoder eagerly materializes the routing header into owned Rust values, but keeps
/// the payload section borrowed so callers can choose which concrete payload type to decode.
pub struct ParsedFrame<'a> {
    header: PacketHeader,
    payload_bytes: &'a [u8],
}

impl<'a> ParsedFrame<'a> {
    #[must_use]
    /// Returns the decoded packet header.
    pub fn header(&self) -> &PacketHeader {
        &self.header
    }

    #[must_use]
    /// Returns the packet class from the decoded header.
    pub fn packet_type(&self) -> PacketType {
        self.header.packet_type
    }

    #[must_use]
    /// Returns the borrowed payload section bytes.
    pub fn payload_bytes(&self) -> &'a [u8] {
        self.payload_bytes
    }

    #[must_use]
    /// Splits the parsed frame into its owned header and borrowed payload bytes.
    pub fn into_parts(self) -> (PacketHeader, &'a [u8]) {
        (self.header, self.payload_bytes)
    }

    /// Deserializes the payload section as a [`CallMessage`].
    pub fn deserialize_call(&self) -> Result<CallMessage, FrameError> {
        self.deserialize_payload::<ArchivedCallMessage, CallMessage>()
    }

    /// Deserializes the payload section as a [`DataMessage`].
    pub fn deserialize_data(&self) -> Result<DataMessage, FrameError> {
        self.deserialize_payload::<ArchivedDataMessage, DataMessage>()
    }

    /// Deserializes the payload section as a [`FaultMessage`].
    pub fn deserialize_fault(&self) -> Result<FaultMessage, FrameError> {
        self.deserialize_payload::<ArchivedFaultMessage, FaultMessage>()
    }

    fn deserialize_payload<A, T>(&self) -> Result<T, FrameError>
    where
        A: rkyv::Portable
            + for<'b> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'b, Error>>,
        T: rkyv::Archive,
        A: rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<Error>>,
    {
        deserialize_archived_bytes::<A, T>(self.payload_bytes)
    }
}

/// Encodes a packet header and payload using the aligned two-section frame format.
///
/// The frame starts with two big-endian `u32` lengths, followed by an aligned archived header
/// section and an aligned archived payload section. Both sections use [`SECTION_ALIGN`] so the
/// archived bytes can usually be accessed without a fallback copy on decode.
pub fn encode_packet<P>(header: &PacketHeader, payload: &P) -> Result<FrameBytes, FrameError>
where
    P: for<'a> Serialize<
        rkyv::api::high::HighSerializer<AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, Error>,
    >,
{
    let header_start = align_up(8usize, SECTION_ALIGN);
    // Reserve enough space for the framing prefix plus a typical header/payload pair so the
    // common encode path avoids early growth reallocations inside `to_bytes_in`.
    let mut frame = FrameBytes::with_capacity(header_start + 256);
    frame.resize(header_start, 0);
    frame = to_bytes_in::<_, Error>(header, frame).map_err(FrameError::Serialize)?;
    let header_len =
        u32::try_from(frame.len() - header_start).map_err(|_| FrameError::LengthOverflow)?;

    let payload_start = align_up(frame.len(), SECTION_ALIGN);
    frame.resize(payload_start, 0);
    frame = to_bytes_in::<_, Error>(payload, frame).map_err(FrameError::Serialize)?;
    let payload_len =
        u32::try_from(frame.len() - payload_start).map_err(|_| FrameError::LengthOverflow)?;

    frame[0..4].copy_from_slice(&header_len.to_be_bytes());
    frame[4..8].copy_from_slice(&payload_len.to_be_bytes());
    Ok(frame)
}

/// Decodes one aligned two-section frame.
///
/// This rejects trailing bytes instead of silently ignoring them, so callers can treat one byte
/// slice as exactly one protocol frame.
pub fn decode_frame(bytes: &[u8]) -> Result<ParsedFrame<'_>, FrameError> {
    let (header_bytes, payload_bytes) = split_frame_sections(bytes)?;
    let header = deserialize_section::<ArchivedPacketHeader, PacketHeader>(
        header_bytes,
        FrameError::InvalidHeader,
    )?;

    Ok(ParsedFrame {
        header,
        payload_bytes,
    })
}

/// Deserializes one archived byte section.
///
/// Payload bytes normally come from [`decode_frame`] or one of [`ParsedFrame`]`'s`
/// `deserialize_*` helpers. This function remains public for callers that archive nested
/// application payloads inside protocol `data` fields.
pub fn deserialize_archived_bytes<A, T>(bytes: &[u8]) -> Result<T, FrameError>
where
    A: rkyv::Portable
        + for<'b> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'b, Error>>,
    T: rkyv::Archive,
    A: rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<Error>>,
{
    deserialize_section::<A, T>(bytes, FrameError::InvalidPayload)
}

fn read_u32(bytes: &[u8], start: usize) -> Result<u32, FrameError> {
    let end = start + 4;
    Ok(u32::from_be_bytes(
        bytes
            .get(start..end)
            .ok_or(FrameError::Truncated)?
            .try_into()
            .expect("slice width checked"),
    ))
}

fn split_frame_sections(bytes: &[u8]) -> Result<(&[u8], &[u8]), FrameError> {
    if bytes.len() < 8 {
        return Err(FrameError::Truncated);
    }

    let header_len = read_u32(bytes, 0)? as usize;
    let payload_len = read_u32(bytes, 4)? as usize;
    let header_start = align_up(8usize, SECTION_ALIGN);
    let header_end = header_start + header_len;
    if header_end > bytes.len() {
        return Err(FrameError::Truncated);
    }

    let payload_start = align_up(header_end, SECTION_ALIGN);
    let payload_end = payload_start + payload_len;
    if payload_end != bytes.len() {
        // Framed packets do not permit trailing bytes. Treating the slice as exactly one frame
        // keeps stream framing bugs visible instead of silently accepting concatenated payloads.
        return Err(FrameError::Truncated);
    }

    Ok((
        bytes
            .get(header_start..header_end)
            .ok_or(FrameError::Truncated)?,
        bytes
            .get(payload_start..payload_end)
            .ok_or(FrameError::Truncated)?,
    ))
}

fn align_up(offset: usize, alignment: usize) -> usize {
    let mask = alignment - 1;
    (offset + mask) & !mask
}

fn deserialize_section<A, T>(
    bytes: &[u8],
    invalid: fn(Error) -> FrameError,
) -> Result<T, FrameError>
where
    A: rkyv::Portable
        + for<'b> rkyv::bytecheck::CheckBytes<rkyv::api::high::HighValidator<'b, Error>>,
    T: rkyv::Archive,
    A: rkyv::Deserialize<T, rkyv::api::high::HighDeserializer<Error>>,
{
    if is_aligned_for::<A>(bytes) {
        let archived = access::<A, Error>(bytes).map_err(invalid)?;
        return deserialize::<T, Error>(archived).map_err(invalid);
    }

    // Archived types may require stronger alignment than a borrowed byte slice can guarantee.
    // Copy into an aligned buffer so callers can still decode valid frames from arbitrary input
    // sources instead of rejecting them purely for allocation layout reasons.
    let mut aligned: FrameBytes = FrameBytes::with_capacity(bytes.len());
    aligned.extend_from_slice(bytes);
    let archived = access::<A, Error>(&aligned).map_err(invalid)?;
    deserialize::<T, Error>(archived).map_err(invalid)
}

fn is_aligned_for<A>(bytes: &[u8]) -> bool {
    let alignment = mem::align_of::<A>();
    alignment <= 1 || (bytes.as_ptr() as usize).is_multiple_of(alignment)
}
