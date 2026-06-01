extern crate alloc;

use alloc::vec::Vec;

use crate::protocol::{DeserializeError, SerializeError};

/// Fully decoded UnShell test packet.
///
/// The current protocol tests route only on hook id, hook end state, and absolute
/// path. `procedure_id` is therefore a compact numeric contract id instead of a
/// string label; application code can maintain its own id-to-name table outside the
/// hot packet path if it needs human-readable names.
#[derive(Debug, Clone)]
pub struct Packet {
    pub hook_id: u16,
    pub end_hook: bool,
    pub path: Vec<u32>,
    pub procedure_id: u32,
    pub data: Vec<u8>,
}

impl Packet {
    /// Serializes the packet into the crate's current little-endian frame format.
    ///
    /// Layout:
    /// - fixed header: `hook_id: u16`, `flags: u8`, padding, `path_len: u32`
    /// - path: `path_len` little-endian `u32` segments
    /// - body: `body_len: u32`, `procedure_id: u32`, raw `data`
    ///
    /// Keeping `procedure_id` fixed-width removes the old string length and UTF-8
    /// validation path. That makes deserialization a single full-packet parse,
    /// which matches how the endpoint mock transports actually consume packets.
    pub fn serialize(&self) -> Result<Vec<u8>, SerializeError> {
        let mut buf = Vec::new();
        self.serialize_into(&mut buf)?;
        Ok(buf)
    }

    /// Appends this packet's serialized frame to an existing byte buffer.
    ///
    /// Transports use this to avoid allocating a temporary frame only to copy it into
    /// their socket write buffer. The method performs all size checks before writing so
    /// serialization errors do not leave a partial frame in `buf`.
    pub fn serialize_into(&self, buf: &mut Vec<u8>) -> Result<(), SerializeError> {
        let path_len = u32::try_from(self.path.len()).map_err(|_| SerializeError::PathTooLarge)?;

        // body = fixed procedure_id field + data bytes
        let body_payload_len = 4usize
            .checked_add(self.data.len())
            .ok_or(SerializeError::BodyTooLarge)?;
        let body_len = u32::try_from(body_payload_len).map_err(|_| SerializeError::BodyTooLarge)?;

        let path_bytes = self
            .path
            .len()
            .checked_mul(4)
            .ok_or(SerializeError::PathTooLarge)?;
        let total = 8usize
            .checked_add(path_bytes)
            .and_then(|n| n.checked_add(4))
            .and_then(|n| n.checked_add(body_payload_len))
            .ok_or(SerializeError::BodyTooLarge)?;

        buf.reserve(total);

        // ── header ────────────────────────────────────────────────────────────
        let flags = self.end_hook as u8;
        buf.extend_from_slice(&self.hook_id.to_le_bytes());
        buf.push(flags);
        buf.push(0u8); // padding
        buf.extend_from_slice(&path_len.to_le_bytes());
        for &segment in &self.path {
            buf.extend_from_slice(&segment.to_le_bytes());
        }

        // ── body ──────────────────────────────────────────────────────────────
        buf.extend_from_slice(&body_len.to_le_bytes());
        buf.extend_from_slice(&self.procedure_id.to_le_bytes());
        buf.extend_from_slice(&self.data);

        Ok(())
    }

    /// Deserializes a full packet from untrusted transport bytes.
    ///
    /// This parser intentionally consumes the complete packet shape. The old
    /// partial parse path was removed because current routing tests and mock
    /// transports always deserialize before calling endpoint routing, so keeping a
    /// borrowed header API only preserved unused unsafe casting complexity.
    #[inline(never)]
    pub fn deserialize(buf: &[u8]) -> Result<Self, DeserializeError> {
        // fixed prefix: hook_id (2) + flags (1) + padding (1) + path_len (4)
        if buf.len() < 8 {
            return Err(DeserializeError::BufferTooShort);
        }

        let hook_id = u16::from_le_bytes([buf[0], buf[1]]);
        let flags = buf[2];
        let end_hook = flags & 0b0000_0001 != 0;
        let path_len = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]) as usize;

        let path_start = 8usize;
        let path_end = path_start
            .checked_add(path_len * 4)
            .ok_or(DeserializeError::PathTooLong)?;

        if buf.len() < path_end {
            return Err(DeserializeError::BufferTooShort);
        }

        let mut path = Vec::with_capacity(path_len);
        for chunk in buf[path_start..path_end].chunks_exact(4) {
            path.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
        }

        // body_len prefix
        let body_buf = &buf[path_end..];
        if body_buf.len() < 4 {
            return Err(DeserializeError::BufferTooShort);
        }
        let body_len =
            u32::from_le_bytes([body_buf[0], body_buf[1], body_buf[2], body_buf[3]]) as usize;

        let body_end = 4usize
            .checked_add(body_len)
            .ok_or(DeserializeError::BodyLengthMismatch)?;
        if body_buf.len() < body_end {
            return Err(DeserializeError::BodyLengthMismatch);
        }

        // procedure_id + data
        let inner = &body_buf[4..body_end];
        if inner.len() < 4 {
            return Err(DeserializeError::BufferTooShort);
        }
        let procedure_id = u32::from_le_bytes([inner[0], inner[1], inner[2], inner[3]]);

        let data = inner[4..].to_vec();

        Ok(Self {
            hook_id,
            end_hook,
            path,
            procedure_id,
            data,
        })
    }
}
