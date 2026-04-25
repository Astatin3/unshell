//! Framed packet encoding and decoding.
//!
//! This module provides the `FrameCodec` trait, which abstracts the conversion
//! between owned packet structures and the canonical length-prefixed wire format.

use alloc::{boxed::Box, vec::Vec};
use core::fmt;
use rkyv::{Serialize, access, deserialize, rancor::Error, to_bytes, util::AlignedVec};

use super::types::{
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

impl core::error::Error for FrameError {}

/// A view into a framed packet, providing access to archived sections.
pub struct ParsedFrame<'a> {
    header: PacketHeader,
    payload_bytes: &'a [u8],
}

impl<'a> ParsedFrame<'a> {
    /// Returns the deserialized packet header.
    ///
    /// The header is owned by `ParsedFrame` because decoding must validate it
    /// before any routing decision is made.
    #[must_use]
    pub fn header(&self) -> &PacketHeader {
        &self.header
    }

    /// Returns the header packet type for quick dispatch.
    #[must_use]
    pub fn packet_type(&self) -> PacketType {
        self.header.packet_type
    }

    /// Returns the raw archived payload section.
    #[must_use]
    pub fn payload_bytes(&self) -> &'a [u8] {
        self.payload_bytes
    }

    /// Clones the decoded header out of the parsed frame.
    #[must_use]
    pub fn deserialize_header(&self) -> PacketHeader {
        self.header.clone()
    }

    /// Consumes the parsed frame and returns its owned header and borrowed payload.
    #[must_use]
    pub fn into_parts(self) -> (PacketHeader, &'a [u8]) {
        (self.header, self.payload_bytes)
    }

    /// Deserializes the payload as a [`CallMessage`].
    pub fn deserialize_call(&self) -> Result<CallMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedCallMessage, CallMessage>(self.payload_bytes)
    }

    /// Deserializes the payload as a [`DataMessage`].
    pub fn deserialize_data(&self) -> Result<DataMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedDataMessage, DataMessage>(self.payload_bytes)
    }

    /// Deserializes the payload as a [`FaultMessage`].
    pub fn deserialize_fault(&self) -> Result<FaultMessage, FrameError> {
        deserialize_archived_bytes::<ArchivedFaultMessage, FaultMessage>(self.payload_bytes)
    }
}

/// Trait for framing and unframing packets.
pub trait FrameCodec {
    /// Encodes a packet header and payload into the canonical framed representation.
    fn encode_packet<P>(header: &PacketHeader, payload: &P) -> Result<FrameBytes, FrameError>
    where
        P: for<'a> Serialize<
            rkyv::api::high::HighSerializer<
                AlignedVec,
                rkyv::ser::allocator::ArenaHandle<'a>,
                Error,
            >,
        >;

    /// Decodes a framed packet into a borrowed parsed view.
    fn decode_frame(bytes: &[u8]) -> Result<ParsedFrame<'_>, FrameError>;
}

/// Default implementation of the `FrameCodec` using `rkyv`.
pub struct RkyvCodec;

impl FrameCodec for RkyvCodec {
    fn encode_packet<P>(header: &PacketHeader, payload: &P) -> Result<FrameBytes, FrameError>
    where
        P: for<'a> Serialize<
            rkyv::api::high::HighSerializer<
                AlignedVec,
                rkyv::ser::allocator::ArenaHandle<'a>,
                Error,
            >,
        >,
    {
        // WARNING: framed packets move as one contiguous buffer across the core boundary.
        // Keeping ownership here avoids hidden copies later in routing code.
        let header_bytes = to_bytes::<Error>(header).map_err(FrameError::Serialize)?;
        let payload_bytes = to_bytes::<Error>(payload).map_err(FrameError::Serialize)?;
        let header_len =
            u32::try_from(header_bytes.len()).map_err(|_| FrameError::LengthOverflow)?;
        let payload_len =
            u32::try_from(payload_bytes.len()).map_err(|_| FrameError::LengthOverflow)?;

        let mut frame = Vec::with_capacity(8 + header_bytes.len() + payload_bytes.len());
        frame.extend_from_slice(&header_len.to_be_bytes());
        frame.extend_from_slice(&header_bytes);
        frame.extend_from_slice(&payload_len.to_be_bytes());
        frame.extend_from_slice(&payload_bytes);
        Ok(frame.into_boxed_slice())
    }

    fn decode_frame(bytes: &[u8]) -> Result<ParsedFrame<'_>, FrameError> {
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
        let header = deserialize::<PacketHeader, Error>(archived_header)
            .map_err(FrameError::InvalidHeader)?;

        Ok(ParsedFrame {
            header,
            payload_bytes: bytes
                .get(payload_start..payload_end)
                .ok_or(FrameError::Truncated)?,
        })
    }
}

/// Encodes a packet header and payload using the default codec.
pub fn encode_packet<P>(header: &PacketHeader, payload: &P) -> Result<FrameBytes, FrameError>
where
    P: for<'a> Serialize<
        rkyv::api::high::HighSerializer<AlignedVec, rkyv::ser::allocator::ArenaHandle<'a>, Error>,
    >,
{
    RkyvCodec::encode_packet(header, payload)
}

/// Decodes a framed packet using the default codec.
pub fn decode_frame(bytes: &[u8]) -> Result<ParsedFrame<'_>, FrameError> {
    RkyvCodec::decode_frame(bytes)
}

/// Deserializes a standalone archived byte section.
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
    // The framed wire format prefixes each archived section with a 4-byte length,
    // so callers cannot rely on the borrowed slice meeting rkyv's alignment.
    // Copying into `AlignedVec` keeps the alignment fix local and predictable.
    let mut aligned = AlignedVec::with_capacity(bytes.len());
    aligned.extend_from_slice(bytes);
    aligned
}
