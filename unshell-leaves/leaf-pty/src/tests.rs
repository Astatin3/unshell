use alloc::{vec, vec::Vec};

use unshell::protocol::{Endpoint, Leaf, Packet};

use super::{
    FakePtyLeaf, FakePtyState, OP_ABORT, OP_ERROR, OP_EXIT, OP_INPUT, OP_OPENED, OP_OUTPUT,
    OP_STDIN_EOF, OP_TERMINATE, PROC_PTY, frame_opcode, frame_payload, pty_open_packet, pty_packet,
};

const ENDPOINT_A: u32 = 0;
const ENDPOINT_B: u32 = 1;
const PROC_OTHER: u32 = 31;

/// Creates a bare endpoint at a known absolute path.
fn endpoint_at(id: u32, path: Vec<u32>) -> Endpoint {
    let mut endpoint = Endpoint::new(id, vec![]);
    endpoint.path = path;
    endpoint
}

/// Creates the parent/child endpoint pair used by PTY session tests.
fn pty_endpoints() -> (Endpoint, Endpoint) {
    let mut endpoint_a = endpoint_at(ENDPOINT_A, vec![ENDPOINT_A]);
    let mut endpoint_b = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);

    endpoint_a.connections.insert((ENDPOINT_B, false));
    endpoint_b.connections.insert((ENDPOINT_A, true));

    (endpoint_a, endpoint_b)
}

/// Transfers every queued packet for `next_hop` into `receiver` as `remote_id` traffic.
fn transfer_packets(sender: &mut Endpoint, receiver: &mut Endpoint, next_hop: u32, remote_id: u32) {
    let mut packets = Vec::new();
    sender.take_outbound_clear(next_hop, |packet| packets.push(packet.clone()));

    for packet in packets {
        receiver.add_inbound_from(remote_id, packet).unwrap();
    }
}

/// Sends one downward PTY frame from endpoint A to endpoint B.
fn send_downward_frame(
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
fn open_pty_session(
    endpoint_a: &mut Endpoint,
    endpoint_b: &mut Endpoint,
    leaf: &mut FakePtyLeaf,
) -> u16 {
    let hook_id = endpoint_a.get_hook_id();
    endpoint_a
        .add_outbound(pty_open_packet(
            vec![ENDPOINT_A, ENDPOINT_B],
            hook_id,
            &[ENDPOINT_A],
        ))
        .unwrap();

    transfer_packets(endpoint_a, endpoint_b, ENDPOINT_B, ENDPOINT_A);
    leaf.update(endpoint_b);
    transfer_packets(endpoint_b, endpoint_a, ENDPOINT_A, ENDPOINT_B);

    hook_id
}

/// Drains PTY packets delivered to endpoint A.
fn drain_parent_pty_packets(endpoint: &mut Endpoint) -> Vec<Packet> {
    let mut packets = Vec::new();
    endpoint.take_inbound_matching(
        ENDPOINT_A,
        |packet| packet.procedure_id == PROC_PTY,
        |packet| packets.push(packet),
    );
    packets
}

/// Asserts that local hook state still contains `hook_id`.
fn assert_hook_present(endpoint: &Endpoint, hook_id: u16) {
    assert!(endpoint.has_hook(hook_id));
}

/// Asserts that local hook state no longer contains `hook_id`.
fn assert_hook_removed(endpoint: &Endpoint, hook_id: u16) {
    assert!(!endpoint.has_hook(hook_id));
}

/// Asserts that `packet` carries the expected PTY frame.
fn assert_frame(packet: &Packet, hook_id: u16, opcode: u8, end_hook: bool, payload: &[u8]) {
    assert_eq!(packet.hook_id, hook_id);
    assert_eq!(packet.end_hook, end_hook);
    assert_eq!(frame_opcode(packet), Some(opcode));
    assert_eq!(frame_payload(packet), payload);
}

/// Returns true when `packets` contains the requested frame.
fn has_frame(packets: &[Packet], hook_id: u16, opcode: u8, payload: &[u8]) -> bool {
    packets.iter().any(|packet| {
        packet.hook_id == hook_id
            && frame_opcode(packet) == Some(opcode)
            && frame_payload(packet) == payload
    })
}

#[test]
fn open_pty_paves_hook_and_creates_session() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());

    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(leaf.active_session_count(), 1);
    assert_eq!(leaf.state().active_count, 1);
    assert_eq!(leaf.state().total_opened, 1);
    assert_hook_present(&endpoint_a, hook_id);
    assert_hook_present(&endpoint_b, hook_id);
    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_OPENED, false, &[]);
}

#[test]
fn input_and_output_share_one_hook() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_INPUT,
        b"hello",
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_OUTPUT, false, b"hello");
    assert_hook_present(&endpoint_a, hook_id);
    assert_hook_present(&endpoint_b, hook_id);
}

#[test]
fn stdin_eof_keeps_hook_until_exit() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_STDIN_EOF,
        &[],
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);

    assert_eq!(leaf.state().last_stdin_eof_hook, Some(hook_id));
    assert!(drain_parent_pty_packets(&mut endpoint_a).is_empty());
    assert_hook_present(&endpoint_a, hook_id);
    assert_hook_present(&endpoint_b, hook_id);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_TERMINATE,
        &[],
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_EXIT, true, &[0]);
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn exit_end_hook_cleans_route_and_session() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_TERMINATE,
        &[],
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_EXIT, true, &[0]);
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn failed_final_exit_route_retries_without_losing_session() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_TERMINATE,
        &[],
        false,
    );
    endpoint_b.connections.remove(&(ENDPOINT_A, true));
    leaf.update(&mut endpoint_b);

    assert_eq!(leaf.active_session_count(), 1);
    assert_eq!(leaf.pending_packet_count(), 1);
    assert_hook_present(&endpoint_b, hook_id);

    endpoint_b.connections.insert((ENDPOINT_A, true));
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_EXIT, true, &[0]);
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn abort_downward_end_hook_closes_without_ack() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_ABORT,
        &[],
        true,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);

    assert_eq!(leaf.active_session_count(), 0);
    assert!(drain_parent_pty_packets(&mut endpoint_a).is_empty());
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn unknown_session_input_returns_error_end_hook() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    let hook_id = endpoint_a.get_hook_id();

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        hook_id,
        OP_INPUT,
        b"orphan",
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(packets.len(), 1);
    assert_frame(&packets[0], hook_id, OP_ERROR, true, b"unknown-session");
    assert_eq!(leaf.active_session_count(), 0);
    assert_hook_removed(&endpoint_a, hook_id);
    assert_hook_removed(&endpoint_b, hook_id);
}

#[test]
fn two_pty_sessions_interleave_without_crossing_hooks() {
    let (mut endpoint_a, mut endpoint_b) = pty_endpoints();
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());

    let first_hook = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    let second_hook = open_pty_session(&mut endpoint_a, &mut endpoint_b, &mut leaf);
    drain_parent_pty_packets(&mut endpoint_a);

    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        second_hook,
        OP_INPUT,
        b"second",
        false,
    );
    send_downward_frame(
        &mut endpoint_a,
        &mut endpoint_b,
        first_hook,
        OP_INPUT,
        b"first",
        false,
    );
    leaf.update(&mut endpoint_b);
    transfer_packets(&mut endpoint_b, &mut endpoint_a, ENDPOINT_A, ENDPOINT_B);
    let packets = drain_parent_pty_packets(&mut endpoint_a);

    assert_eq!(leaf.active_session_count(), 2);
    assert_eq!(packets.len(), 2);
    assert!(has_frame(&packets, first_hook, OP_OUTPUT, b"first"));
    assert!(has_frame(&packets, second_hook, OP_OUTPUT, b"second"));
    assert_hook_present(&endpoint_a, first_hook);
    assert_hook_present(&endpoint_a, second_hook);
    assert_hook_present(&endpoint_b, first_hook);
    assert_hook_present(&endpoint_b, second_hook);
}

#[test]
fn pty_leaf_does_not_consume_other_leaf_packets() {
    let mut endpoint = endpoint_at(ENDPOINT_B, vec![ENDPOINT_A, ENDPOINT_B]);
    let mut leaf = FakePtyLeaf::new(FakePtyState::new());
    endpoint.connections.insert((ENDPOINT_A, true));

    endpoint
        .add_inbound_from(
            ENDPOINT_A,
            pty_open_packet(vec![ENDPOINT_A, ENDPOINT_B], 7, &[ENDPOINT_A]),
        )
        .unwrap();
    endpoint
        .add_inbound_from(
            ENDPOINT_A,
            Packet {
                hook_id: 8,
                end_hook: false,
                path: vec![ENDPOINT_A, ENDPOINT_B],
                procedure_id: PROC_OTHER,
                data: b"leave-me".to_vec(),
            },
        )
        .unwrap();

    leaf.update(&mut endpoint);

    let mut other_packets = Vec::new();
    endpoint.take_inbound_matching(
        ENDPOINT_B,
        |packet| packet.procedure_id == PROC_OTHER,
        |packet| other_packets.push(packet),
    );

    assert_eq!(leaf.active_session_count(), 1);
    assert_eq!(other_packets.len(), 1);
    assert_eq!(other_packets[0].procedure_id, PROC_OTHER);
    assert_eq!(other_packets[0].data, b"leave-me".to_vec());
}
