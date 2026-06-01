use alloc::{vec, vec::Vec};

use unshell::protocol::{Leaf, Packet};

use crate::{
    FakePtyLeaf, FakePtyState, OP_ABORT, OP_ERROR, OP_EXIT, OP_INPUT, OP_OUTPUT, OP_STDIN_EOF,
    OP_TERMINATE, pty_open_packet,
};

use super::support::{
    ENDPOINT_A, ENDPOINT_B, PROC_OTHER, assert_frame, assert_hook_present, assert_hook_removed,
    assert_opened, drain_parent_pty_packets, endpoint_at, has_frame, open_pty_session,
    pty_endpoints, send_downward_frame, transfer_packets,
};

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
    assert_opened(&packets[0], hook_id);
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
