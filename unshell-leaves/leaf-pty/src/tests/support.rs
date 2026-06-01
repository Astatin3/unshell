use alloc::{vec, vec::Vec};

use unshell::protocol::{Endpoint, Leaf, Packet};

use crate::{
    FakePtyLeaf, OP_OPENED, PROC_PTY, frame_opcode, frame_payload, pty_open_packet, pty_packet,
};

pub(super) const ENDPOINT_A: u32 = 0;
pub(super) const ENDPOINT_B: u32 = 1;
pub(super) const PROC_OTHER: u32 = 31;

/// Creates a bare endpoint at a known absolute path.
pub(super) fn endpoint_at(id: u32, path: Vec<u32>) -> Endpoint {
    let mut endpoint = Endpoint::new(id, vec![]);
    endpoint.path = path;
    endpoint
}

/// Creates the parent/child endpoint pair used by PTY session tests.
pub(super) fn pty_endpoints() -> (Endpoint, Endpoint) {
    let mut endpoint_a = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    let mut endpoint_b = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);

    endpoint_a.connections.insert((ENDPOINT_B, false));
    endpoint_b.connections.insert((ENDPOINT_A, true));

    (endpoint_a, endpoint_b)
}

/// Transfers every queued packet for `next_hop` into `receiver` as `remote_id` traffic.
pub(super) fn transfer_packets(
    sender: &mut Endpoint,
    receiver: &mut Endpoint,
    next_hop: u32,
    remote_id: u32,
) {
    let mut packets = Vec::new();
    sender.take_outbound_clear(next_hop, |packet| packets.push(packet.clone()));

    for packet in packets {
        receiver.add_inbound_from(remote_id, packet).unwrap();
    }
}

/// Sends one downward PTY frame from endpoint A to endpoint B.
pub(super) fn send_downward_frame(
    endpoint_a: &mut Endpoint,
    endpoint_b: &mut Endpoint,
    hook_id: u16,
    opcode: u8,
    payload: &[u8],
    end_hook: bool,
) {
    endpoint_a
        .add_outbound(pty_packet(
            vec![ENDPOINT_A, ENDPOINT_B],
            hook_id,
            end_hook,
            opcode,
            payload,
        ))
        .unwrap();
    transfer_packets(endpoint_a, endpoint_b, ENDPOINT_B, ENDPOINT_A);
}

/// Opens a fake PTY session and delivers the `Opened` response to endpoint A.
pub(super) fn open_pty_session(
    endpoint_a: &mut Endpoint,
    endpoint_b: &mut Endpoint,
    leaf: &mut FakePtyLeaf,
) -> u16 {
    let hook_id = endpoint_a.get_hook_id();
    endpoint_a
        .add_outbound(pty_open_packet(vec![ENDPOINT_A, ENDPOINT_B], hook_id))
        .unwrap();

    transfer_packets(endpoint_a, endpoint_b, ENDPOINT_B, ENDPOINT_A);
    leaf.update(endpoint_b);
    transfer_packets(endpoint_b, endpoint_a, ENDPOINT_A, ENDPOINT_B);

    hook_id
}

/// Drains packets for `procedure_id` delivered to endpoint A.
pub(super) fn drain_parent_packets(endpoint: &mut Endpoint, procedure_id: u32) -> Vec<Packet> {
    let mut packets = Vec::new();
    endpoint.take_inbound_matching(
        ENDPOINT_A,
        |packet| packet.procedure_id == procedure_id,
        |packet| packets.push(packet),
    );
    packets
}

/// Drains PTY packets delivered to endpoint A.
pub(super) fn drain_parent_pty_packets(endpoint: &mut Endpoint) -> Vec<Packet> {
    drain_parent_packets(endpoint, PROC_PTY)
}

/// Asserts that local hook state still contains `hook_id`.
pub(super) fn assert_hook_present(endpoint: &Endpoint, hook_id: u16) {
    assert!(endpoint.has_hook(hook_id));
}

/// Asserts that local hook state no longer contains `hook_id`.
pub(super) fn assert_hook_removed(endpoint: &Endpoint, hook_id: u16) {
    assert!(!endpoint.has_hook(hook_id));
}

/// Asserts that `packet` carries the expected PTY frame.
pub(super) fn assert_frame(
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
pub(super) fn has_frame(packets: &[Packet], hook_id: u16, opcode: u8, payload: &[u8]) -> bool {
    packets.iter().any(|packet| {
        packet.hook_id == hook_id
            && frame_opcode(packet) == Some(opcode)
            && frame_payload(packet) == payload
    })
}

/// Asserts that a packet is the fake PTY open acknowledgement.
pub(super) fn assert_opened(packet: &Packet, hook_id: u16) {
    assert_frame(packet, hook_id, OP_OPENED, false, &[]);
}
