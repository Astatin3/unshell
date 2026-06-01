use unshell::protocol::{Endpoint, Packet};

use crate::{OP_OPENED, frame_opcode, frame_payload};

/// Asserts that local hook state still contains `hook_id`.
pub(crate) fn assert_hook_present(endpoint: &Endpoint, hook_id: u16) {
    assert!(
        endpoint.has_hook(hook_id),
        "expected hook {hook_id} to remain registered"
    );
}

/// Asserts that local hook state no longer contains `hook_id`.
pub(crate) fn assert_hook_removed(endpoint: &Endpoint, hook_id: u16) {
    assert!(
        !endpoint.has_hook(hook_id),
        "expected hook {hook_id} to be cleaned up"
    );
}

/// Asserts that `packet` carries the expected PTY frame.
pub(crate) fn assert_frame(
    packet: &Packet,
    hook_id: u16,
    opcode: u8,
    end_hook: bool,
    payload: &[u8],
) {
    assert_eq!(packet.hook_id, hook_id);
    assert_eq!(packet.end_hook, end_hook);
    assert_eq!(frame_opcode(packet), Some(opcode));
    assert_eq!(frame_payload(packet), payload);
}

/// Returns true when `packets` contains the requested frame.
pub(crate) fn has_frame(packets: &[Packet], hook_id: u16, opcode: u8, payload: &[u8]) -> bool {
    packets.iter().any(|packet| {
        packet.hook_id == hook_id
            && frame_opcode(packet) == Some(opcode)
            && frame_payload(packet) == payload
    })
}

/// Asserts that a packet is the fake PTY open acknowledgement.
pub(crate) fn assert_opened(packet: &Packet, hook_id: u16) {
    assert_frame(packet, hook_id, OP_OPENED, false, &[]);
}
