use alloc::vec::Vec;

use unshell::protocol::{HookID, Packet};

use crate::{OP_ERROR, OP_OPEN, PROC_PTY};

/// Encodes a tiny PTY frame into `Packet::data`.
pub fn encode_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + payload.len());
    data.push(opcode);
    data.extend_from_slice(payload);
    data
}

/// Encodes an `Open` payload with the caller's reply path.
pub fn encode_open(reply_path: &[u32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(2 + reply_path.len() * 4);
    data.push(OP_OPEN);
    data.push(reply_path.len() as u8);

    for segment in reply_path {
        data.extend_from_slice(&segment.to_le_bytes());
    }

    data
}

/// Decodes the reply path embedded in an `Open` payload after the opcode byte.
pub fn decode_open_reply_path(payload: &[u8]) -> Option<Vec<u32>> {
    let path_len = usize::from(*payload.first()?);
    let path_bytes = path_len.checked_mul(4)?;
    let expected_len = 1usize.checked_add(path_bytes)?;

    if payload.len() != expected_len {
        return None;
    }

    let mut path = Vec::with_capacity(path_len);
    for chunk in payload[1..].chunks_exact(4) {
        path.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }

    Some(path)
}

/// Returns the opcode byte from a PTY packet, if present.
pub fn frame_opcode(packet: &Packet) -> Option<u8> {
    packet.data.first().copied()
}

/// Returns the frame payload after the opcode byte.
pub fn frame_payload(packet: &Packet) -> &[u8] {
    if packet.data.len() > 1 {
        &packet.data[1..]
    } else {
        &[]
    }
}

/// Builds an outer PTY packet for callers and tests.
pub fn pty_packet(
    path: Vec<u32>,
    hook_id: HookID,
    end_hook: bool,
    opcode: u8,
    payload: &[u8],
) -> Packet {
    Packet {
        hook_id,
        end_hook,
        path,
        procedure_id: PROC_PTY,
        data: encode_frame(opcode, payload),
    }
}

/// Builds an outer PTY open packet with the specialized open payload shape.
pub fn pty_open_packet(path: Vec<u32>, hook_id: HookID, reply_path: &[u32]) -> Packet {
    Packet {
        hook_id,
        end_hook: false,
        path,
        procedure_id: PROC_PTY,
        data: encode_open(reply_path),
    }
}

/// Builds a final error packet for session initialization failures.
pub(crate) fn error_packet(hook_id: HookID, reply_path: Vec<u32>, payload: &[u8]) -> Packet {
    Packet {
        hook_id,
        end_hook: true,
        path: reply_path,
        procedure_id: PROC_PTY,
        data: encode_frame(OP_ERROR, payload),
    }
}

/// Infers the caller reply path from a locally delivered destination path.
pub(crate) fn reply_path_from_destination(destination: &[u32]) -> Vec<u32> {
    if destination.len() > 1 {
        destination[..destination.len() - 1].to_vec()
    } else {
        destination.to_vec()
    }
}
