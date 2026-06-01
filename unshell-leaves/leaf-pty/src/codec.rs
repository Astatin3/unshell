use alloc::vec::Vec;

use unshell::protocol::{HookID, Packet};

use crate::{OP_OPEN, PROC_PTY};

/// Encodes a tiny PTY frame into `Packet::data`.
pub fn encode_frame(opcode: u8, payload: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + payload.len());
    data.push(opcode);
    data.extend_from_slice(payload);
    data
}

/// Encodes an `Open` frame.
pub fn encode_open() -> Vec<u8> {
    alloc::vec![OP_OPEN]
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

/// Builds an outer PTY open packet.
pub fn pty_open_packet(path: Vec<u32>, hook_id: HookID) -> Packet {
    Packet {
        hook_id,
        end_hook: false,
        path,
        procedure_id: PROC_PTY,
        data: encode_open(),
    }
}
