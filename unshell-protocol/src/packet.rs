extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use crate::{DeserializeError, SerializeError};

#[derive(Debug)]
pub struct Packet {
    pub hook_id: u16,
    pub end_hook: bool,
    pub path: Vec<u32>,
    // ── body (routers never read below this line) ──
    pub procedure_id: String,
    pub data: Vec<u8>,
}

/// Returned by `deserialize_header` — only what a router needs.
/// `body_remainder` is a raw slice into the original buffer so the
/// entire body can be forwarded without touching it.
#[derive(Debug)]
pub struct HeaderRef<'buf> {
    pub hook_id: u16,
    pub end_hook: bool,
    pub path: &'buf [u32],
    pub body_remainder: &'buf [u8],
}

impl Packet {
    pub fn serialize(&self) -> Result<Vec<u8>, SerializeError> {
        let proc_id_bytes = self.procedure_id.as_bytes();

        let path_len = self.path.len() as u32;
        let proc_id_len =
            u32::try_from(proc_id_bytes.len()).map_err(|_| SerializeError::ProcIdTooLarge)?;

        // body = proc_id_len field + proc_id bytes + data bytes
        let body_payload_len = 4usize
            .checked_add(proc_id_bytes.len())
            .and_then(|n| n.checked_add(self.data.len()))
            .ok_or(SerializeError::BodyTooLarge)?;
        let body_len = u32::try_from(body_payload_len).map_err(|_| SerializeError::BodyTooLarge)?;

        let total = 8 + (self.path.len() * 4) + 4 + body_payload_len;
        let mut buf = Vec::with_capacity(total);

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
        buf.extend_from_slice(&proc_id_len.to_le_bytes());
        buf.extend_from_slice(proc_id_bytes);
        buf.extend_from_slice(&self.data);

        Ok(buf)
    }

    /// Deserialize only the header — O(path_len), body bytes are never read.
    /// A router can inspect `HeaderRef` then forward the original buffer as-is.
    pub fn deserialize_header(buf: &[u8]) -> Result<HeaderRef<'_>, DeserializeError> {
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

        // Cast the buffer slice to a u32 slice.
        // This requires alignment. rkyv handles this, but for a manual cast:
        let path_ptr = buf[path_start..path_end].as_ptr() as *const u32;
        let path = unsafe { core::slice::from_raw_parts(path_ptr, path_len) };

        Ok(HeaderRef {
            hook_id,
            end_hook,
            path,
            body_remainder: &buf[path_end..],
        })
    }

    /// Full deserialization. Parses the header then the body.
    pub fn deserialize(buf: &[u8]) -> Result<Self, DeserializeError> {
        let header = Self::deserialize_header(buf)?;
        let body_buf = header.body_remainder;

        // body_len prefix
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

        // proc_id_len + proc_id
        let inner = &body_buf[4..body_end];
        if inner.len() < 4 {
            return Err(DeserializeError::BufferTooShort);
        }
        let proc_id_len = u32::from_le_bytes([inner[0], inner[1], inner[2], inner[3]]) as usize;

        let proc_id_start = 4usize;
        let proc_id_end = proc_id_start
            .checked_add(proc_id_len)
            .ok_or(DeserializeError::ProcIdTooLong)?;
        if inner.len() < proc_id_end {
            return Err(DeserializeError::BufferTooShort);
        }

        let procedure_id = core::str::from_utf8(&inner[proc_id_start..proc_id_end])
            .map_err(|_| DeserializeError::InvalidUtf8)?;

        let data = inner[proc_id_end..].to_vec();

        Ok(Self {
            hook_id: header.hook_id,
            end_hook: header.end_hook,
            path: header.path.to_vec(),
            procedure_id: procedure_id.into(),
            data,
        })
    }
}
