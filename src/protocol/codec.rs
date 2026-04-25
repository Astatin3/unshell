//! Framed packet encoding and decoding.
use core::{fmt, mem};
use rkyv::{Serialize, access, deserialize, rancor::Error, to_bytes, util::AlignedVec};

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
    Truncated,
    InvalidHeader(Error),
    InvalidPayload(Error),
    Serialize(Error),
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
pub struct ParsedFrame<'a> {
    header: PacketHeader,
    payload_bytes: &'a [u8],
}

impl<'a> ParsedFrame<'a> {
    #[must_use]
    pub fn header(&self) -> &PacketHeader {
        &self.header
    }

    #[must_use]
    pub fn packet_type(&self) -> PacketType {
        self.header.packet_type
    }

    #[must_use]
    pub fn payload_bytes(&self) -> &'a [u8] {
        self.payload_bytes
    }

    #[must_use]
    pub fn into_parts(self) -> (PacketHeader, &'a [u8]) {
        (self.header, self.payload_bytes)
    }

    pub fn deserialize_call(&self) -> Result<CallMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedCallMessage, CallMessage>(self.payload_bytes)
    }

    pub fn deserialize_data(&self) -> Result<DataMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedDataMessage, DataMessage>(self.payload_bytes)
    }

    pub fn deserialize_fault(&self) -> Result<FaultMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedFaultMessage, FaultMessage>(self.payload_bytes)
    }
}

/// Encodes a packet header and payload using the aligned two-section frame format.
pub fn encode_packet<P>(header: &PacketHeader, payload: &P) -> Result<FrameBytes, FrameError>
where
    P: for<'a> Serialize<
        rkyv::api::high::HighSerializer<AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, Error>,
    >,
{
    let header_bytes: FrameBytes = to_bytes::<Error>(header).map_err(FrameError::Serialize)?;
    let payload_bytes: FrameBytes = to_bytes::<Error>(payload).map_err(FrameError::Serialize)?;
    let header_len = u32::try_from(header_bytes.len()).map_err(|_| FrameError::LengthOverflow)?;
    let payload_len = u32::try_from(payload_bytes.len()).map_err(|_| FrameError::LengthOverflow)?;

    let header_start = 8usize;
    let payload_start = align_up(header_start + header_bytes.len(), SECTION_ALIGN);
    let total_len = payload_start + payload_bytes.len();

    let mut frame = FrameBytes::with_capacity(total_len);
    frame.extend_from_slice(&header_len.to_be_bytes());
    frame.extend_from_slice(&payload_len.to_be_bytes());
    frame.extend_from_slice(&header_bytes);
    append_padding(
        &mut frame,
        payload_start - (header_start + header_bytes.len()),
    );
    frame.extend_from_slice(&payload_bytes);
    Ok(frame)
}

/// Decodes one aligned two-section frame.
pub fn decode_frame(bytes: &[u8]) -> Result<ParsedFrame<'_>, FrameError> {
    if bytes.len() < 8 {
        return Err(FrameError::Truncated);
    }

    let header_len = read_u32(bytes, 0)? as usize;
    let payload_len = read_u32(bytes, 4)? as usize;
    let header_start = 8usize;
    let header_end = header_start + header_len;
    if header_end > bytes.len() {
        return Err(FrameError::Truncated);
    }

    let payload_start = align_up(header_end, SECTION_ALIGN);
    let payload_end = payload_start + payload_len;
    if payload_end != bytes.len() {
        return Err(FrameError::Truncated);
    }

    let header = deserialize_section::<ArchivedPacketHeader, PacketHeader>(
        bytes
            .get(header_start..header_end)
            .ok_or(FrameError::Truncated)?,
        FrameError::InvalidHeader,
    )?;

    Ok(ParsedFrame {
        header,
        payload_bytes: bytes
            .get(payload_start..payload_end)
            .ok_or(FrameError::Truncated)?,
    })
}

/// Deserializes one archived byte section.
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

fn append_padding(frame: &mut AlignedVec, padding: usize) {
    if padding > 0 {
        frame.resize(frame.len() + padding, 0);
    }
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

    let mut aligned: FrameBytes = FrameBytes::with_capacity(bytes.len());
    aligned.extend_from_slice(bytes);
    let archived = access::<A, Error>(&aligned).map_err(invalid)?;
    deserialize::<T, Error>(archived).map_err(invalid)
}

fn is_aligned_for<A>(bytes: &[u8]) -> bool {
    let alignment = mem::align_of::<A>();
    alignment <= 1 || (bytes.as_ptr() as usize).is_multiple_of(alignment)
}
